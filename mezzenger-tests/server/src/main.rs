use std::cell::RefCell;
use std::env::current_dir;
use std::sync::Arc;
use std::sync::Mutex;

use anyhow::Result;
use futures::{future::FutureExt, pin_mut, select, SinkExt, StreamExt};
use kodec::binary::Codec;
use mezzenger::{Messages, Receive};
use tokio::{
    signal::ctrl_c,
    spawn,
    sync::oneshot::{self, Sender},
};
use tracing::{error, info, Level};
use warp::{hyper::StatusCode, ws::WebSocket, Filter};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Server running!");

    let (browser_tests_passed_sender, browser_tests_passed_receiver) = oneshot::channel::<()>();
    let (native_tests_passed_sender, native_tests_passed_receiver) = oneshot::channel::<()>();

    let browser_tests_passed_sender = Arc::new(Mutex::new(Some(browser_tests_passed_sender)));
    let native_tests_passed_sender = Arc::new(Mutex::new(Some(native_tests_passed_sender)));
    let native_tests_counter = Arc::new(Mutex::new(RefCell::new(0)));

    let current_dir = current_dir()?;
    info!("Current working directory: {:?}", current_dir);

    let browser_tests_notifier = warp::any().map(move || browser_tests_passed_sender.clone());
    let native_tests_notifier = warp::any().map(move || native_tests_passed_sender.clone());
    let native_tests_counter = warp::any().map(move || native_tests_counter.clone());

    let static_files = warp::get().and(warp::fs::dir("www"));
    let websocket = warp::path("ws")
        .and(warp::ws())
        .and(browser_tests_notifier)
        .and(native_tests_notifier)
        .and(native_tests_counter)
        .map(
            |ws: warp::ws::Ws,
             browser_tests_notifier,
             native_tests_notifier,
             native_tests_counter| {
                ws.on_upgrade(move |socket| {
                    handle_websocket(
                        socket,
                        browser_tests_notifier,
                        native_tests_notifier,
                        native_tests_counter,
                    )
                })
            },
        );
    let routes = websocket.or(static_files).recover(handle_rejection);

    let (address, server_future) =
        warp::serve(routes).bind_with_graceful_shutdown(([127, 0, 0, 1], 3030), async move {
            let termination = ctrl_c().fuse();
            let tests_passed = async move {
                browser_tests_passed_receiver.await.unwrap();
                native_tests_passed_receiver.await.unwrap();
            }
            .fuse();

            pin_mut!(termination, tests_passed);

            select! {
                result = termination => result.expect("unable to listen for shutdown signal"),
                _ = tests_passed => info!("All tests passed!"),
            }
        });
    let server_handle = spawn(server_future);
    info!("Listening at {}...", address);

    server_handle.await?;
    info!("Shutting down...");

    Ok(())
}

async fn handle_websocket(
    web_socket: WebSocket,
    browser_tests_notifier: Arc<Mutex<Option<Sender<()>>>>,
    native_tests_notifier: Arc<Mutex<Option<Sender<()>>>>,
    native_tests_counter: Arc<Mutex<RefCell<usize>>>,
) {
    info!("Opening transport...");
    let codec = Codec::default();
    let (mut sender, mut receiver) =
        mezzenger_websocket::warp::Transport::<_, Codec, common::Message2, common::Message1>::new(
            web_socket, codec,
        )
        .split();
    info!("Transport open.");

    let native_client = match receiver.receive().await.unwrap() {
        common::Message2::Welcome { native_client } => native_client,
        _ => {
            error!("received unexpected message");
            panic!();
        }
    };

    if native_client {
        info!("Native client connected.");
    } else {
        info!("Browser client connected.");
    }

    info!("Sending...");
    for message in common::messages1_part1().iter() {
        sender.send(message.clone()).await.unwrap();
    }

    for message in common::messages1_part2().iter() {
        sender.send(message.clone()).await.unwrap();
    }
    info!("Messages sent.");

    assert_eq!(
        common::messages2_all(),
        receiver.messages().collect::<Vec<common::Message2>>().await
    );

    if native_client {
        let mut native_tests_counter = native_tests_counter.lock().unwrap();
        *native_tests_counter.get_mut() += 1;
        info!(
            "Native client test {}/2 passed.",
            *native_tests_counter.get_mut()
        );
        if *native_tests_counter.get_mut() >= 2 {
            if let Some(notifier) = native_tests_notifier.lock().unwrap().take() {
                notifier.send(()).unwrap();
            }
        }
    } else {
        info!("Browser client tests passed.");
        if let Some(notifier) = browser_tests_notifier.lock().unwrap().take() {
            notifier.send(()).unwrap();
        }
    }
}

async fn handle_rejection(
    err: warp::Rejection,
) -> Result<impl warp::Reply, std::convert::Infallible> {
    if err.is_not_found() {
        error!("Error occurred: {:?}", err);
        Ok(warp::reply::with_status("Not found", StatusCode::NOT_FOUND))
    } else {
        error!("Error occurred: {:?}", err);
        Ok(warp::reply::with_status(
            "Internal server error",
            StatusCode::INTERNAL_SERVER_ERROR,
        ))
    }
}
