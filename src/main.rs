mod adapters;
mod ai;
mod chrome;
mod cli;
mod cli_commands;
mod config;
mod domain;
mod extract;
mod ports;
mod report;
mod store;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    cli::run().await
}
