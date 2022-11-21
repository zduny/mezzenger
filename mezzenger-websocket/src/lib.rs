//! Transport for communication over
//! [WebSocket](https://developer.mozilla.org/en-US/docs/Web/API/WebSocket).
//!
//! Provides implementations for:
//! - browsers,
//! - native applications (through [tokio-tungstenite](https://github.com/snapview/tokio-tungstenite)),
//! - [warp](https://github.com/seanmonstar/warp) servers (enabled with `warp` feature).
//!
//! See [repository](https://github.com/zduny/mezzenger) for more info.

#[cfg(all(feature = "warp", not(target_arch = "wasm32")))]
pub mod warp;

#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
mod native;
#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
pub use native::*;

#[cfg(all(feature = "wasm", target_arch = "wasm32"))]
mod wasm;
#[cfg(all(feature = "wasm", target_arch = "wasm32"))]
pub use wasm::*;
