mod client;
mod server;

use anyhow::Result;
use clap::Parser;

#[derive(Debug, Parser)]
struct Args {
    /// Run as server (as opposed to client).
    #[arg(short, long)]
    server: bool,

    /// Server URL.
    #[arg(short, long, default_value = "127.0.0.1:8080")]
    url: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    if args.server {
        server::run(&args.url).await?;
    } else {
        client::run(&args.url).await?;
    }

    Ok(())
}
