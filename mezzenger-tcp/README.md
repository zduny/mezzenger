# mezzenger-tcp

[![Crate](https://img.shields.io/crates/v/mezzenger-tcp.svg)](https://crates.io/crates/mezzenger-tcp)
[![API](https://docs.rs/mezzenger-tcp/badge.svg)](https://docs.rs/mezzenger-tcp)

TCP transport for [mezzenger](https://github.com/zduny/mezzenger).

https://crates.io/crates/mezzenger-tcp

[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/O5O31JYZ4)

## note

Despite its name this crate supports other transports as well as long as they implement 
`AsyncRead` and `AsyncWrite` traits.

## usage

Add dependencies to `Cargo.toml`:

```toml
[dependencies]
# ...
serde = { version = "1", features = ["derive"] }
kodec = { version = "0.1.0", features = ["binary"] } # or json or different one from another crate...
mezzenger = "0.1.3"
mezzenger-tcp = "0.1.1"
```

See example code [here](https://github.com/zduny/mezzenger/tree/master/mezzenger-tcp/examples/chat).

## see also

[mezzenger](https://github.com/zduny/mezzenger)
