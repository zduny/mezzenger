mod client;
mod server;

use anyhow::Result;
use clap::Parser;

#[derive(Debug, Parser)]
struct Args {
    /// Run as server (as opposed to client).
    #[arg(short, long)]
    server: bool,

    /// Server address.
    #[arg(short, long, default_value = "127.0.0.1:8080")]
    address: String,

    /// Use IPC instead of TCP
    #[arg(short, long)]
    ipc: bool,

    /// Use IPC instead of TCP
    #[arg(short, long, default_value = "/tmp/chat.ipc")]
    path: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let Args {
        server,
        address,
        ipc,
        path,
    } = Args::parse();

    if server {
        server::run(ipc, &address, &path).await?;
    } else {
        client::run(ipc, &address, &path).await?;
    }

    Ok(())
}
