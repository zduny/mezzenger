use std::rc::Rc;
use wasm_bindgen::prelude::*;
use web_sys::Worker;
use js_utils::{console_log, set_panic_hook};
use kodec::binary::Codec;
use futures::{SinkExt, StreamExt};
use mezzenger::Messages;
use mezzenger_webworker::Transport;

pub async fn test_webworker()  {
    console_log!("Starting worker...");
    let worker = Rc::new(Worker::new("./worker.js").unwrap());
    let mut transport: Transport<_, Codec, common::Message1, common::Message2> =
        Transport::new(&worker, Codec::default()).await.unwrap();
    console_log!("Transport open.");

    console_log!("Sending...");
    for message in common::messages2_part1().iter() {
        transport.send(message).await.unwrap();
    }

    for message in common::messages2_part2().iter() {
        transport.send(message).await.unwrap();
    }
    console_log!("Messages sent.");

    assert_eq!(common::messages1_all(), transport.messages().collect::<Vec<common::Message1>>().await);
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

    Ok(())
}
