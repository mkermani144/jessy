use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_tungstenite::{
    connect_async, tungstenite::protocol::Message, MaybeTlsStream, WebSocketStream,
};
use url::Url;

#[derive(Debug, Clone)]
pub struct CdpClient {
    http: reqwest::Client,
    debug_endpoint: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BrowserVersion {
    #[serde(rename = "Browser")]
    pub browser: String,
    #[serde(rename = "Protocol-Version")]
    pub protocol_version: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PageTarget {
    pub id: String,
    #[serde(rename = "type")]
    pub target_type: String,
    pub url: String,
    #[serde(rename = "webSocketDebuggerUrl")]
    pub websocket_debugger_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct CdpRequest {
    id: u64,
    method: String,
    params: Value,
}

pub struct CdpSession {
    stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
    next_id: u64,
}

impl CdpClient {
    pub fn new(debug_endpoint: String) -> Self {
        Self {
            http: reqwest::Client::new(),
            debug_endpoint,
        }
    }

    pub async fn version(&self) -> Result<BrowserVersion> {
        let url = format!("{}/json/version", self.debug_endpoint);
        let response = self
            .http
            .get(&url)
            .send()
            .await
            .context("failed to query Chrome /json/version")?
            .error_for_status()
            .context("Chrome /json/version returned error status")?;

        response
            .json::<BrowserVersion>()
            .await
            .context("failed to parse /json/version")
    }

    pub async fn list_tabs(&self) -> Result<Vec<PageTarget>> {
        let url = format!("{}/json/list", self.debug_endpoint);
        let response = self
            .http
            .get(&url)
            .send()
            .await
            .context("failed to query Chrome /json/list")?
            .error_for_status()
            .context("Chrome /json/list returned error status")?;

        let all = response
            .json::<Vec<PageTarget>>()
            .await
            .context("failed to parse /json/list")?;

        Ok(all
            .into_iter()
            .filter(|t| t.target_type == "page" && t.websocket_debugger_url.is_some())
            .collect())
    }

    pub async fn open_tab(&self, url: &str) -> Result<PageTarget> {
        let encoded = Url::parse(url)
            .map(|u| u.to_string())
            .unwrap_or_else(|_| url.to_string());

        let endpoint = format!("{}/json/new?{}", self.debug_endpoint, encoded);
        let response = self
            .http
            .put(endpoint)
            .send()
            .await
            .context("failed to open new Chrome tab")?
            .error_for_status()
            .context("Chrome /json/new returned error")?;

        response
            .json::<PageTarget>()
            .await
            .context("failed to parse /json/new response")
    }

    pub async fn close_tab(&self, tab_id: &str) -> Result<()> {
        let endpoint = format!("{}/json/close/{tab_id}", self.debug_endpoint);
        self.http
            .get(endpoint)
            .send()
            .await
            .context("failed to close Chrome tab")?
            .error_for_status()
            .context("Chrome /json/close returned error")?;

        Ok(())
    }
}

impl CdpSession {
    pub async fn connect(websocket_url: &str) -> Result<Self> {
        let (stream, _resp) = connect_async(websocket_url)
            .await
            .with_context(|| format!("failed websocket connect to {websocket_url}"))?;

        Ok(Self { stream, next_id: 1 })
    }

    pub async fn enable_basics(&mut self) -> Result<()> {
        let _ = self.call("Page.enable", json!({})).await?;
        let _ = self.call("Runtime.enable", json!({})).await?;
        Ok(())
    }

    pub async fn navigate(&mut self, url: &str) -> Result<()> {
        let _ = self.call("Page.navigate", json!({ "url": url })).await?;
        self.sleep_for_page().await;
        Ok(())
    }

    pub async fn evaluate(&mut self, expression: &str) -> Result<Value> {
        let response = self
            .call(
                "Runtime.evaluate",
                json!({
                    "expression": expression,
                    "returnByValue": true,
                    "awaitPromise": true,
                }),
            )
            .await?;

        if response.get("exceptionDetails").is_some() {
            bail!("Runtime.evaluate raised an exception")
        }

        let value = response
            .get("result")
            .and_then(|r| r.get("value"))
            .cloned()
            .unwrap_or(Value::Null);

        Ok(value)
    }

    async fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;

        let request = CdpRequest {
            id,
            method: method.to_string(),
            params,
        };

        let serialized =
            serde_json::to_string(&request).context("failed to serialize CDP request")?;
        self.stream
            .send(Message::Text(serialized.into()))
            .await
            .context("failed to send CDP request")?;

        let result = timeout(Duration::from_secs(20), async {
            while let Some(message) = self.stream.next().await {
                let message = message.context("failed receiving CDP response")?;
                match message {
                    Message::Text(text) => {
                        let value: Value = serde_json::from_str(&text)
                            .context("failed to deserialize CDP message")?;

                        if value.get("id").and_then(|v| v.as_u64()) == Some(id) {
                            if let Some(err) = value.get("error") {
                                return Err(anyhow!("CDP error: {err}"));
                            }
                            return Ok(value.get("result").cloned().unwrap_or(Value::Null));
                        }
                    }
                    Message::Binary(bin) => {
                        if let Ok(text) = String::from_utf8(bin.to_vec()) {
                            let value: Value = serde_json::from_str(&text)
                                .context("failed to deserialize binary CDP message")?;
                            if value.get("id").and_then(|v| v.as_u64()) == Some(id) {
                                if let Some(err) = value.get("error") {
                                    return Err(anyhow!("CDP error: {err}"));
                                }
                                return Ok(value.get("result").cloned().unwrap_or(Value::Null));
                            }
                        }
                    }
                    Message::Ping(payload) => {
                        self.stream.send(Message::Pong(payload)).await?;
                    }
                    Message::Close(_) => break,
                    _ => {}
                }
            }

            Err(anyhow!(
                "CDP socket closed before response for method {method}"
            ))
        })
        .await
        .context("CDP timeout waiting for response")??;

        Ok(result)
    }

    async fn sleep_for_page(&self) {
        tokio::time::sleep(Duration::from_millis(1300)).await;
    }
}
