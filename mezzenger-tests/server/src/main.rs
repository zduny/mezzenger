use std::env::current_dir;

use anyhow::Result;
use tokio::{signal::ctrl_c, spawn};
use tracing::{error, info, Level};
use warp::{hyper::StatusCode, Filter};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Server running!");

    let current_dir = current_dir()?;
    info!("Current working directory: {:?}", current_dir);

    let static_files = warp::get().and(warp::fs::dir("www"));
    let routes = static_files.recover(handle_rejection);

    let (address, server_future) =
        warp::serve(routes).bind_with_graceful_shutdown(([127, 0, 0, 1], 3030), async move {
            ctrl_c()
                .await
                .expect("unable to listen for shutdown signal");
        });
    let server_handle = spawn(server_future);
    info!("Listening at {}...", address);

    server_handle.await?;
    info!("Shutting down...");

    Ok(())
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
