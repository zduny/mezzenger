# mezzenger-utils

[![Crate](https://img.shields.io/crates/v/mezzenger-utils.svg)](https://crates.io/crates/mezzenger-utils)
[![API](https://docs.rs/mezzenger-utils/badge.svg)](https://docs.rs/mezzenger-utils)

Utils for [mezzenger](https://github.com/zduny/mezzenger).

https://crates.io/crates/mezzenger-utils

[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/O5O31JYZ4)

## available utilities

Following utilities are available or staged for development:

- `Inspector` - wrapper transport calling a callback whenever it sends or receives a message.<br>
  **Work in progress**.

- `Split` - split transport into two with different message types.<br>
  **Work in progress**.

- `Merged` - merge [futures](https://github.com/rust-lang/futures-rs) `Stream` and `Sink`
  into a `mezzenger` transport.<br>
  **Work in progress**.

- `Numbered` - wrapper transport attaching a number to messages.

- `LatestOnly` - wrapper transport turning a numbered (but not necessarily ordered) transport
  into an ordered transport, discarding old messages (polling a transport for the next message will return the latest received message, ignoring messages received before).<br>
  Potentially useful when user doesn't care about stale messages (for example multiplayer video games).

- `Reliable` - wrapper turning unreliable transport into reliable one (by acknowledging and resending lost messages after timeout).<br>
  **Work in progress**.

- `Ordered` - wrapper turning unordered (not guaranteeing message order)
but reliable transport into ordered (and deduplicating) one.<br>
  **Work in progress**.

- `Unreliabler` - wrapper turning a transport into unreliable (possibly losing messages) and/or unordered (sending messages not in order, possibly duplicating messages) one - useful for testing.<br>
  **Work in progress**. 

## see also

[mezzenger](https://github.com/zduny/mezzenger)
