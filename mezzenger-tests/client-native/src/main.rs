use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use futures::{stream, SinkExt, StreamExt};
use kodec::binary::Codec;
use mezzenger::{Messages, Receive};
use tokio::time::sleep;
use tokio_tungstenite::connect_async;
use url::Url;

/// Mezzenger tests native client
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Server URL.
    #[arg(short, long, default_value = "ws://localhost:3030/ws")]
    url: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    println!("Hello World!");

    let url = Url::parse(&args.url)?;
    let (web_socket, _) = connect_async(&url).await?;

    println!("Opening transport...");
    let codec = Codec::default();
    let (mut sender, mut receiver) =
        mezzenger_websocket::Transport::<_, Codec, common::Message1, common::Message2>::new(
            web_socket, codec,
        )
        .split();
    println!("Transport open.");

    println!("Sending welcome message...");
    sender
        .send(common::Message2::Welcome {
            native_client: true,
        })
        .await
        .unwrap();
    println!("Welcome message sent.");

    let messages = common::messages1_all();

    assert_eq!(receiver.receive().await.unwrap(), messages[0]);

    println!("Sending...");
    sender
        .send_all(&mut stream::iter(
            common::messages2_all().into_iter().map(Ok),
        ))
        .await
        .unwrap();
    println!("Messages sent.");

    sleep(Duration::from_secs(1)).await;

    println!("Closing transport...");
    sender.close().await.unwrap();
    println!("Transport closed.");

    assert_eq!(
        messages
            .into_iter()
            .skip(1)
            .collect::<Vec<common::Message1>>(),
        receiver.messages().collect::<Vec<common::Message1>>().await
    );

    println!("Testing abrupt close...");

    let (web_socket, _) = connect_async(&url).await?;

    println!("Opening transport...");
    let codec = Codec::default();
    let (mut sender, mut receiver) =
        mezzenger_websocket::Transport::<_, Codec, common::Message1, common::Message2>::new(
            web_socket, codec,
        )
        .split();
    println!("Transport open.");

    println!("Sending welcome message...");
    sender
        .send(common::Message2::Welcome {
            native_client: true,
        })
        .await
        .unwrap();
    println!("Welcome message sent.");

    let messages = common::messages1_all();

    assert_eq!(receiver.receive().await.unwrap(), messages[0]);

    println!("Sending...");
    sender
        .send_all(&mut stream::iter(
            common::messages2_all().into_iter().map(Ok),
        ))
        .await
        .unwrap();
    println!("Messages sent.");

    sleep(Duration::from_secs(1)).await;

    println!("Dropping transport...");
    drop(sender);
    drop(receiver);
    println!("Transport dropped.");

    println!("Tests passed.");

    Ok(())
}
