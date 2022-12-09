# chat

Simple chat application.

## usage

Navigate to `mezzenger-tcp` root directory.

First, start server:
```bash
cargo run --example chat -- --server
```

Start client (in another terminal window):
```bash
cargo run --example chat
```

## ipc

Example also supports IPC communication (using [parity-tokio-ipc](https://github.com/paritytech/parity-tokio-ipc) crate).

Use `--ipc` argument to use IPC instead of TCP:

```bash
cargo run --example chat -- --server --ipc
```

```bash
cargo run --example chat -- --ipc
```

## see also

[rustyline-async](https://github.com/zyansheep/rustyline-async) - crate by [zyansheep](https://github.com/zyansheep)
used in this example to create a command promt interface.

[parity-tokio-ipc](https://github.com/paritytech/parity-tokio-ipc) - interprocess transport for UNIX/Windows.
