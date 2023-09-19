# mezzenger-udp

[![Crate](https://img.shields.io/crates/v/mezzenger-udp.svg)](https://crates.io/crates/mezzenger-udp)
[![API](https://docs.rs/mezzenger-udp/badge.svg)](https://docs.rs/mezzenger-udp)

UDP transport for [mezzenger](https://github.com/zduny/mezzenger).

https://crates.io/crates/mezzenger-udp

[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/O5O31JYZ4)

## usage

Add dependencies to `Cargo.toml`:

```toml
[dependencies]
# ...
serde = { version = "1", features = ["derive"] }
kodec = { version = "0.1.0", features = ["binary"] } # or json or different one from another crate...
mezzenger = "0.1.3"
mezzenger-udp = "0.1.2"
```

Now, in code:

```rust
let udp_socket = UdpSocket::bind("127.0.0.1:8080").await?;
udp_socket.connect(remote_address).await?;

use kodec::binary::Codec;
let mut transport: Transport<_, Codec, i32, String> =
    Transport::new(udp_socket, Codec::default());

use mezzenger::Receive;
let integer = transport.receive().await?;

transport.send("Hello World!".to_string()).await?;
```

## see also

[mezzenger](https://github.com/zduny/mezzenger)
