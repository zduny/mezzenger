# mezzenger-tcp

TCP transport for [mezzenger](https://github.com/zduny/mezzenger).

https://crates.io/crates/mezzenger-tcp

[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/O5O31JYZ4)

## usage

Add dependencies to `Cargo.toml`:

```toml
[dependencies]
# ...
serde = { version = "1", features = ["derive"] }
kodec = { version = "0.1.0", features = ["binary"] } # or json or different one from another crate...
mezzenger = "0.1.2"
mezzenger-tcp = "0.1.1"
```

See example code here.

## see also

[mezzenger](https://github.com/zduny/mezzenger)
