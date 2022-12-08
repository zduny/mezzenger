# mezzenger

Message passing infrastructure for Rust.

https://crates.io/crates/mezzenger

[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/O5O31JYZ4)

## transport implementations

| Create                                                                                                       |    Native    |    Browser    | Description                                                                                                            |
|:-------------------------------------------------------------------------------------------------------------|:------------:|:-------------:|:-----------------------------------------------------------------------------------------------------------------------|
| [mezzenger-tcp](https://github.com/zduny/mezzenger/tree/master/mezzenger-tcp)                                | ✅           | *n/a*        | Transport over [Tokio](https://tokio.rs/) TCP implementation.                                                          |
| [mezzenger-udp](https://github.com/zduny/mezzenger/tree/master/mezzenger-udp)                                | ✅           | *n/a*        | Transport over [Tokio](https://tokio.rs/) UDP implementation.                                                          |
| [mezzenger-webworker](https://github.com/zduny/mezzenger/tree/master/mezzenger-webworker)                    | *n/a*        | ✅             | Communication with [Web Workers](https://developer.mozilla.org/en-US/docs/Web/API/Web_Workers_API/Using_web_workers).  |
| [mezzenger-websocket](https://github.com/zduny/mezzenger/tree/master/mezzenger-websocket)                    | ✅           | ✅             | Transport over [WebSockets](https://developer.mozilla.org/en-US/docs/Web/API/WebSockets_API).                          |
| [mezzenger-webrtc-datachannel](https://github.com/zduny/mezzenger/tree/master/mezzenger-webrtc-datachannel)  | *to do*      | *wip*         | Transport over [WebRTC data channel](https://developer.mozilla.org/en-US/docs/Web/API/WebRTC_API/Using_data_channels). |


## description

The goal of `mezzenger` project is to create and maintain a set of crates that make it easy to pass messages
over the network and network-like interfaces.

Other **goals**:
 - maintaining similar interface across transport implementations,
 - providing (where applicable) implementations that work both on native and browser WASM targets,
 - creation of various that make it easy to layer/compose transports of different types and/or properties (see **further work** section).

**Non**-goals:
 - encryption - it should be handled by underlying transport,
 - node.js WASM targets - contributions are welcome, but they won't be developed/maintained by the [author](https://github.com/zduny) of this project,
 - best possible performance - implementations are supposed to be decent, without obvious areas for improvement, but if you need to save every bit of bandwidth you'd likely be better served by a custom application-specific protocol.  

## example

See [rust-webapp-template](https://github.com/zduny/rust-webapp-template).

## further work

Following utils are on roadmap for development:

| Name                       | Description                                                                                                                                                                                                                                |
|:---------------------------|:-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `mezzenger-reliabler`     | Wrapper turning unreliable transport into reliable one <br> (by acknowledging and resending lost messages after timeout).                                                                                                                  |
| `mezzenger-orderer`        | Wrapper turning unordered (not guaranteeing message order) <br> but reliable transport into ordered one.                                                                                                                                   |
| `mezzenger-last-only`      | Wrapper turning unordered (not guaranteeing message order) <br> transport into ordered one, but discarding old messages. <br> Potentially useful when user doesn't care about old messages <br> (for example multiplayer video games). |
| `mezzenger-splitter`     | Split `mezzenger` transport into two of different message types.                                                                                                                  |

## see also

[kodec](https://github.com/zduny/kodec)
