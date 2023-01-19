# mezzenger-utils

Utils for [mezzenger](https://github.com/zduny/mezzenger).

https://crates.io/crates/mezzenger-utils

[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/O5O31JYZ4)

## available utilities

Following utilities are available or staged for development:

- `Inspector` - wrapper transport calling a callback whenever it sends or receives a message.<br>
  **Work in progress**.

- `Splitter` - split transport into two with different message types.<br>
  **Work in progress**.

- `Merger` - merge [futures](https://github.com/rust-lang/futures-rs) `Stream` and `Sink`
  into a `mezzenger` transport.<br>
  **Work in progress**.

- `Numbered` - wrapper transport attaching an ordered number to messages.<br>
  **Work in progress**.

- `LastOnly` - wrapper transport turning a numbered (but not necessarily ordered) transport
  into an ordered transport, discarding old messages (polling a transport for the next message will return the latest received message, ignoring messages received before).<br>
  Potentially useful when user doesn't care about stale messages (for example multiplayer video games).<br>
  **Work in progress**.

- `Reliabler` - wrapper turning unreliable transport into reliable one (by acknowledging and resending lost messages after timeout).<br>
  **Work in progress**.

- `Orderer` - wrapper turning unordered (not guaranteeing message order)
but reliable transport into ordered one.<br>
  **Work in progress**.

- `Unreliabler` - wrapper turning a transport into unreliable (possibly losing messages) and/or unordered (sending messages not in order, possibly duplicating messages) one - useful for testing.<br>
  **Work in progress**. 

## see also

[mezzenger](https://github.com/zduny/mezzenger)
