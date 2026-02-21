use std::process::Stdio;

use anyhow::{bail, Context, Result};
use tokio::{
    process::Command,
    time::{sleep, Duration},
};

use crate::config::ChromeConfig;

pub async fn ensure_debug_chrome(cfg: &ChromeConfig) -> Result<()> {
    if is_debug_port_ready(cfg.debug_port).await {
        return Ok(());
    }

    if !cfg.auto_launch {
        bail!(
            "Chrome remote debugging is not reachable on port {} and auto_launch is disabled",
            cfg.debug_port
        );
    }

    launch_chrome(cfg).await?;

    for _ in 0..15 {
        if is_debug_port_ready(cfg.debug_port).await {
            return Ok(());
        }
        sleep(Duration::from_millis(500)).await;
    }

    bail!(
        "failed to connect to Chrome debug endpoint on port {} after launch",
        cfg.debug_port
    )
}

pub fn debug_endpoint(port: u16) -> String {
    format!("http://127.0.0.1:{port}")
}

async fn launch_chrome(cfg: &ChromeConfig) -> Result<()> {
    let status = Command::new(&cfg.binary_path)
        .arg(format!("--remote-debugging-port={}", cfg.debug_port))
        .arg(format!("--user-data-dir={}", cfg.profile_dir))
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("failed to launch Chrome")?;

    drop(status);
    Ok(())
}

async fn is_debug_port_ready(port: u16) -> bool {
    let url = format!("http://127.0.0.1:{port}/json/version");
    reqwest::Client::new()
        .get(url)
        .send()
        .await
        .map(|resp| resp.status().is_success())
        .unwrap_or(false)
}
