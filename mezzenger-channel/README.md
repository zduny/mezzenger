# mezzenger-channel

[![Crate](https://img.shields.io/crates/v/mezzenger-channel.svg)](https://crates.io/crates/mezzenger-channel)
[![API](https://docs.rs/mezzenger-channel/badge.svg)](https://docs.rs/mezzenger-channel)

Transport for communication over [futures](https://github.com/rust-lang/futures-rs) channels.

https://crates.io/crates/mezzenger-channel

[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/O5O31JYZ4)

## usage

Add dependencies to `Cargo.toml`:

```toml
[dependencies]
# ...
mezzenger = "0.1.3"
mezzenger-channel = "0.1.0"
```

Example code:

```rust
let (mut left, mut right) = transports();

left.send("Hello World!").await.unwrap();
right.send(123).await.unwrap();

use mezzenger::Receive;
assert_eq!(right.receive().await.unwrap(), "Hello World!");
assert_eq!(left.receive().await.unwrap(), 123);
```

## see also

[mezzenger](https://github.com/zduny/mezzenger)
