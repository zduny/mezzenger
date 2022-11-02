use futures::{SinkExt, stream::StreamExt};
use js_utils::{console_log, set_panic_hook, sleep};
use kodec::binary::Codec;
use mezzenger::{Receive, Messages};
use mezzenger_webworker::Transport;
use std::time::Duration;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub async fn main() -> Result<(), JsValue> {
    set_panic_hook();

    console_log!("Worker: worker started!");
    let mut transport: Transport<_, Codec, common::Message2, common::Message1> =
        Transport::new_in_worker(Codec::default()).await.unwrap();

    let messages = common::messages2_all();

    assert_eq!(transport.receive().await.unwrap(), messages[0]);

    sleep(Duration::from_secs(1)).await;

    console_log!("Worker: closing transport...");
    transport.close().await.unwrap();
    console_log!("Worker: transport closed.");

    assert_eq!(
        messages
            .into_iter()
            .skip(1)
            .collect::<Vec<common::Message2>>(),
        transport.messages().collect::<Vec<common::Message2>>().await
    );
    console_log!("Worker: tests passed.");

    Ok(())
}
