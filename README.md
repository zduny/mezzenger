# mezzenger

Message passing infrastructure for Rust.

https://crates.io/crates/mezzenger

[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/O5O31JYZ4)

## transport implementations

| Create                                                                                                       |    Native    |    Browser    | Description                                                                                                            |
|:-------------------------------------------------------------------------------------------------------------|:------------:|:-------------:|:-----------------------------------------------------------------------------------------------------------------------|
| [mezzenger-tcp](https://github.com/zduny/mezzenger/tree/master/mezzenger-tcp)                                | *to do*      | *to do*       | Transport over [Tokio](https://tokio.rs/) TCP implementation.                                                          |
| [mezzenger-udp](https://github.com/zduny/mezzenger/tree/master/mezzenger-udp)                                | *to do*      | *to do*       | Transport over [Tokio](https://tokio.rs/) UDP implementation.                                                          |
| [mezzenger-websocket](https://github.com/zduny/mezzenger/tree/master/mezzenger-websocket)                    | ✅           | ✅             | Transport over [WebSockets](https://developer.mozilla.org/en-US/docs/Web/API/WebSockets_API).                          |
| [mezzenger-webworker](https://github.com/zduny/mezzenger/tree/master/mezzenger-webworker)                    | ✅           | ✅             | Communication with [Web Workers](https://developer.mozilla.org/en-US/docs/Web/API/Web_Workers_API/Using_web_workers).  |
| [mezzenger-webrtc-datachannel](https://github.com/zduny/mezzenger/tree/master/mezzenger-webrtc-datachannel)  | *to do*      | *wip*         | Transport over [WebRTC data channel](https://developer.mozilla.org/en-US/docs/Web/API/WebRTC_API/Using_data_channels). |

## example

See [rust-webapp-template](https://github.com/zduny/rust-webapp-template).

## further work

Following utils are on roadmap for development:

- `mezzegner-reliabler` - wrapper turning unreliable transport into reliable one (by acknowledging and resending lost messages after timeout),
- `mezzenger-orderer`   - wrapper turning unordered (not guaranteeing message order) but reliable transport into ordered one,
- `mezzenger-last-only` - wrapper turning unordered (not guaranteeing message order) transport into ordered one, but discarding old messages - potentially useful when user doesn't care about old messages (for example multiplayer video games),

## see also

[kodec](https://github.com/zduny/kodec)
