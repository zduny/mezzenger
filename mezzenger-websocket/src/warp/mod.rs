//! Transport for communication over
//! [WebSocket](https://developer.mozilla.org/en-US/docs/Web/API/WebSocket)
//! from [warp](https://github.com/seanmonstar/warp) servers.

pub mod receiver;
pub use receiver::Receiver;
pub mod sender;
pub use sender::Sender;
