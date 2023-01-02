# mezzenger-webworker

Transport for communication with [Web Workers](https://developer.mozilla.org/en-US/docs/Web/API/Web_Workers_API).

https://crates.io/crates/mezzenger-webworker

[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/O5O31JYZ4)

## usage

Add dependencies to `Cargo.toml`:

```toml
[dependencies]
# ...
serde = { version = "1", features = ["derive"] }
kodec = { version = "0.1.0", features = ["binary"] } # or json or different one from another crate...
mezzenger = "0.1.3"
mezzenger-webworker = "0.1.0"
```

In your main code:

```rust
mod message {
  #[derive(Debug, Serialize, Deserialize)]
  struct Host {
    ...
  }

  #[derive(Debug, Serialize, Deserialize)]
  struct Worker { 
    ...
  }
}

// ...

let worker = Rc::new(Worker::new("./worker.js").unwrap());
let mut transport: Transport<_, Codec, message::Worker, message::Host> =
        Transport::new(&worker, Codec::default()).await.except("failed to open transport");

use mezzenger::Receive;
let received = transport.receive().await.except("failed to receive message");

let message = message::Host { ... };
transport.send(&message).await.except("failed to send message");

```

In Web Worker:

```rust
mod message {
  #[derive(Debug, Serialize, Deserialize)]
  struct Host {
    ...
  }

  #[derive(Debug, Serialize, Deserialize)]
  struct Worker { 
    ...
  }
}

// ...

let mut transport: Transport<_, Codec, message::Host, message::Worker> =
        Transport::new_in_worker(Codec::default()).await.except("failed to open transport");

let message = message::Worker { ... };
transport.send(&message).await.except("failed to send message");

use mezzenger::Receive;
let received = transport.receive().await.except("failed to receive message");

```

See [rust-webapp-template](https://github.com/zduny/rust-webapp-template) for more comprehensive example.

## see also

[mezzenger](https://github.com/zduny/mezzenger)

[rust-webapp-template](https://github.com/zduny/rust-webapp-template)

[Using Web Workers](https://developer.mozilla.org/en-US/docs/Web/API/Web_Workers_API/Using_web_workers)

[WASM in Web Worker](https://rustwasm.github.io/wasm-bindgen/examples/wasm-in-web-worker.html)
