use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use crate::{
    chrome::{
        cdp_client::{CdpClient, CdpSession},
        launcher,
    },
    config::ChromeConfig,
    ports::browser::{
        BrowserAutomation, BrowserPageTab, BrowserSession, BrowserVersion, CandidateTab,
    },
};

/// Chrome/CDP implementation of `BrowserAutomation`.
pub struct ChromeBrowser {
    cfg: ChromeConfig,
    cdp: CdpClient,
}

impl ChromeBrowser {
    /// Creates a browser adapter bound to the configured debug port/profile.
    pub fn new(cfg: ChromeConfig) -> Self {
        let cdp = CdpClient::new(launcher::debug_endpoint(cfg.debug_port));
        Self { cfg, cdp }
    }
}

/// Thin wrapper converting `CdpSession` to the `BrowserSession` port.
pub struct ChromeSession {
    inner: CdpSession,
}

#[async_trait]
impl BrowserSession for ChromeSession {
    async fn enable_basics(&mut self) -> Result<()> {
        self.inner.enable_basics().await
    }

    async fn navigate(&mut self, url: &str) -> Result<()> {
        self.inner.navigate(url).await
    }

    async fn evaluate(&mut self, expression: &str) -> Result<Value> {
        self.inner.evaluate(expression).await
    }
}

#[async_trait]
impl BrowserAutomation for ChromeBrowser {
    async fn ensure_ready(&self) -> Result<()> {
        launcher::ensure_debug_chrome(&self.cfg).await
    }

    async fn version(&self) -> Result<BrowserVersion> {
        let v = self.cdp.version().await?;
        Ok(BrowserVersion {
            browser: v.browser,
            protocol_version: v.protocol_version,
        })
    }

    async fn list_tabs(&self) -> Result<Vec<BrowserPageTab>> {
        let tabs = self.cdp.list_tabs().await?;
        Ok(tabs
            .into_iter()
            .map(|t| BrowserPageTab {
                id: t.id,
                url: t.url,
                websocket_debugger_url: t.websocket_debugger_url,
            })
            .collect())
    }

    async fn list_candidate_tabs(&self) -> Result<Vec<CandidateTab>> {
        let tabs = self.cdp.list_tabs().await?;
        Ok(tabs
            .into_iter()
            .filter_map(|t| {
                let ws = t.websocket_debugger_url?;
                Some(CandidateTab {
                    url: t.url,
                    websocket_debugger_url: ws,
                })
            })
            .collect())
    }

    async fn open_tab(&self, url: &str) -> Result<BrowserPageTab> {
        let t = self.cdp.open_tab(url).await?;
        Ok(BrowserPageTab {
            id: t.id,
            url: t.url,
            websocket_debugger_url: t.websocket_debugger_url,
        })
    }

    async fn close_tab(&self, tab_id: &str) -> Result<()> {
        self.cdp.close_tab(tab_id).await
    }

    async fn connect_session(&self, websocket_url: &str) -> Result<Box<dyn BrowserSession>> {
        let session = CdpSession::connect(websocket_url).await?;
        Ok(Box::new(ChromeSession { inner: session }))
    }

    fn debug_endpoint(&self) -> String {
        launcher::debug_endpoint(self.cfg.debug_port)
    }

    fn profile_dir(&self) -> &str {
        &self.cfg.profile_dir
    }
}
