use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

/// Browser version metadata exposed by CDP.
#[derive(Debug, Clone)]
pub struct BrowserVersion {
    pub browser: String,
    pub protocol_version: String,
}

/// Browser tab metadata used by scanner orchestration.
#[derive(Debug, Clone)]
pub struct BrowserPageTab {
    pub id: String,
    pub url: String,
    pub websocket_debugger_url: Option<String>,
}

/// Tab candidate accepted by configured source filters.
#[derive(Debug, Clone)]
pub struct CandidateTab {
    pub url: String,
    pub websocket_debugger_url: String,
}

/// Port over a single browser debugging session.
#[async_trait]
pub trait BrowserSession: Send {
    /// Enables required CDP domains (`Page`, `Runtime`, ...).
    async fn enable_basics(&mut self) -> Result<()>;
    /// Navigates current tab to an absolute URL.
    async fn navigate(&mut self, url: &str) -> Result<()>;
    /// Executes JavaScript and returns JSON-serializable value.
    async fn evaluate(&mut self, expression: &str) -> Result<Value>;
}

/// Port for browser lifecycle and tab/session operations.
#[async_trait]
pub trait BrowserAutomation: Send + Sync {
    /// Ensures dedicated debug Chrome is running.
    async fn ensure_ready(&self) -> Result<()>;
    /// Queries browser version/protocol details.
    async fn version(&self) -> Result<BrowserVersion>;
    /// Lists all inspectable page tabs.
    async fn list_tabs(&self) -> Result<Vec<BrowserPageTab>>;
    /// Lists tabs eligible for scanning.
    async fn list_candidate_tabs(&self) -> Result<Vec<CandidateTab>>;
    /// Opens a temporary tab.
    async fn open_tab(&self, url: &str) -> Result<BrowserPageTab>;
    /// Closes a tab by CDP id.
    async fn close_tab(&self, tab_id: &str) -> Result<()>;
    /// Connects a session to a tab websocket endpoint.
    async fn connect_session(&self, websocket_url: &str) -> Result<Box<dyn BrowserSession>>;
    /// Debug endpoint base URL.
    fn debug_endpoint(&self) -> String;
    /// Dedicated profile directory in use.
    fn profile_dir(&self) -> &str;
}
