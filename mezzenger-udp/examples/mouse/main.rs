//! Broadcast (and receive) server's mouse position.

mod client;
mod server;

use anyhow::Result;
use clap::Parser;

#[derive(Debug, Parser)]
struct Args {
    /// Run as server (as opposed to client).
    #[arg(short, long)]
    server: bool,

    /// Broadcast address.
    #[arg(short, long, default_value = "192.168.0.255:1234")]
    url: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    if args.server {
        server::run(&args.url).await?;
    } else {
        client::run().await?;
    }

    Ok(())
}
