use std::rc::Rc;
use std::time::Duration;
use wasm_bindgen::prelude::*;
use web_sys::{Worker, WebSocket};
use js_utils::{console_log, set_panic_hook, window, sleep};
use kodec::binary::Codec;
use futures::{SinkExt, StreamExt, stream};
use mezzenger::{Receive, Messages};

pub async fn test_webworker() {
    use mezzenger_webworker::Transport;

    console_log!("Starting worker...");
    let worker = Rc::new(Worker::new("./worker.js").unwrap());
    let mut transport: Transport<_, Codec, common::Message1, common::Message2> =
        Transport::new(&worker, Codec::default()).await.unwrap();
    console_log!("Transport open.");

    console_log!("Sending...");
    for message in common::messages2_part1().into_iter() {
        transport.send(message).await.unwrap();
    }

    for message in common::messages2_part2().into_iter() {
        transport.send(message).await.unwrap();
    }
    console_log!("Messages sent.");

    assert_eq!(common::messages1_all(), transport.messages().collect::<Vec<common::Message1>>().await);
    console_log!("Tests passed.");
}

pub async fn test_websocket() {
    use mezzenger_websocket::Transport;

    console_log!("Opening WebSocket...");
    let host = window().location().host().expect("couldn't extract host from location");
    let url = format!("ws://{host}/ws");
    let web_socket = Rc::new(WebSocket::new(&url).unwrap());
    let mut transport: Transport<Codec, common::Message1, common::Message2> =
        Transport::new(&web_socket, Codec::default()).await.unwrap();
    console_log!("Transport open.");

    console_log!("Sending welcome message...");
    transport.send(common::Message2::Welcome { native_client: false }).await.unwrap();
    console_log!("Welcome message sent.");

    let messages = common::messages1_all();

    assert_eq!(transport.receive().await.unwrap(), messages[0]);

    console_log!("Sending...");
    transport.send_all(&mut stream::iter(common::messages2_all().into_iter().map(Ok))).await.unwrap();
    console_log!("Messages sent.");

    sleep(Duration::from_secs(1)).await;

    console_log!("Closing transport...");
    transport.close().await.unwrap();
    console_log!("Transport closed.");

    assert_eq!(
        messages
            .into_iter()
            .skip(1)
            .collect::<Vec<common::Message1>>(),
        transport.messages().collect::<Vec<common::Message1>>().await
    );
    console_log!("Tests passed.");
}

#[wasm_bindgen(start)]
pub async fn main() -> Result<(), JsValue> {
    set_panic_hook();

    console_log!("Hello World!");

    console_log!("\n");

    console_log!("Testing Web Worker transport...");
    test_webworker().await;
    console_log!("Web Worker transport test passed!");
    console_log!("\n");

    console_log!("Testing Web Socket transport...");
    test_websocket().await;
    console_log!("Web Socket transport test passed!");

    Ok(())
}
