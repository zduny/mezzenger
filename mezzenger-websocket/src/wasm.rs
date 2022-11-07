//! Transport for communication over
//! [WebSocket](https://developer.mozilla.org/en-US/docs/Web/API/WebSocket).

use std::{
    cell::RefCell,
    collections::VecDeque,
    fmt::{Debug, Display},
    marker::PhantomData,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll, Waker},
};

use futures::{stream::FusedStream, Sink, Stream};
use js_sys::Uint8Array;
use js_utils::{
    event::{EventListener, When},
    JsError, Queue,
};
use kodec::{Decode, Encode};
use serde::Serialize;
use web_sys::{BinaryType, CloseEvent, Event, MessageEvent, WebSocket};

#[derive(Debug)]
pub enum Error<SerializationError, DeserializationError> {
    SendingError(JsError),
    ClosingError(JsError),
    SerializationError(SerializationError),
    DeserializationError(DeserializationError),
    WebSocketError(Event),
}

impl<SerializationError, DeserializationError> Display
    for Error<SerializationError, DeserializationError>
where
    SerializationError: Display,
    DeserializationError: Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::SendingError(error) => write!(f, "failed to send message: {error}"),
            Error::ClosingError(error) => write!(f, "failed to close transport: {error}"),
            Error::SerializationError(error) => write!(f, "failed to serialize message: {error}"),
            Error::DeserializationError(error) => {
                write!(f, "failed to deserialize message: {error}")
            }
            Error::WebSocketError(error) => write!(f, "WebSocket error occurred: {error:?}"),
        }
    }
}

impl<SerializationError, DeserializationError> std::error::Error
    for Error<SerializationError, DeserializationError>
where
    SerializationError: Debug + Display,
    DeserializationError: Debug + Display,
{
}

struct State<Incoming, Error> {
    incoming: VecDeque<Result<Incoming, Error>>,
    waker: Option<Waker>,
    closed: bool,
}

impl<Incoming, Error> State<Incoming, Error> {
    fn new() -> Self {
        State {
            incoming: VecDeque::new(),
            waker: None,
            closed: false,
        }
    }

    fn message(&mut self, message: Incoming) {
        self.incoming.push_back(Ok(message));
        self.wake();
    }

    fn error(&mut self, error: Error) {
        self.incoming.push_back(Err(error));
        self.wake();
    }

    fn close(&mut self) {
        self.closed = true;
        self.wake();
    }

    fn update_waker_with(&mut self, other: &Waker) {
        if let Some(waker) = &self.waker {
            if !waker.will_wake(other) {
                self.waker = Some(other.clone());
            }
        } else {
            self.waker = Some(other.clone());
        }
    }

    fn wake(&mut self) {
        if let Some(waker) = self.waker.take() {
            waker.wake();
        }
    }
}

impl<Incoming, Error> Drop for State<Incoming, Error> {
    fn drop(&mut self) {
        if !self.closed {
            self.close();
        }
    }
}

/// Transport for communication over
/// [WebSocket](https://developer.mozilla.org/en-US/docs/Web/API/WebSocket
pub struct Transport<Codec, Incoming, Outgoing>
where
    Codec: kodec::Codec,
{
    web_socket: Rc<WebSocket>,
    codec: Codec,
    #[allow(clippy::type_complexity)]
    state: Rc<RefCell<State<Incoming, Error<<Codec as Encode>::Error, <Codec as Decode>::Error>>>>,
    buffer: RefCell<Vec<u8>>,
    _message_listener: EventListener<WebSocket, MessageEvent>,
    _error_listener: EventListener<WebSocket, Event>,
    _close_listener: EventListener<WebSocket, CloseEvent>,
    _outgoing: PhantomData<Outgoing>,
}

impl<Codec, Incoming, Outgoing> Transport<Codec, Incoming, Outgoing>
where
    Codec: 'static + kodec::Codec + Clone,
    Incoming: 'static,
    Outgoing: 'static + Serialize,
    <Codec as Encode>::Error: 'static,
    <Codec as Decode>::Error: 'static,
    for<'de> Incoming: serde::de::Deserialize<'de>,
{
    /// Create new transport for WebSocket without waiting for `open` event.
    pub fn new_assuming_open(web_socket: &Rc<WebSocket>, codec: Codec) -> Result<Self, JsError> {
        web_socket.set_binary_type(BinaryType::Arraybuffer);
        let web_socket = web_socket.clone();
        let codec_clone = codec.clone();
        let state = Rc::new(RefCell::new(State::new()));
        let state_clone = state.clone();
        let message_listener = web_socket.when("message", move |event: MessageEvent| {
            let array = Uint8Array::new(&event.data());
            let vector = array.to_vec();
            let result: Result<Incoming, _> = codec_clone.decode(&vector[..]);
            match result {
                Ok(message) => state_clone.borrow_mut().message(message),
                Err(error) => state_clone
                    .borrow_mut()
                    .error(Error::DeserializationError(error)),
            }
        })?;
        let state_clone = state.clone();
        let error_listener = web_socket.when("error", move |event: Event| {
            state_clone.borrow_mut().error(Error::WebSocketError(event));
            state_clone.borrow_mut().close();
        })?;
        let state_clone = state.clone();
        let close_listener = web_socket.when("close", move |_event: CloseEvent| {
            state_clone.borrow_mut().close();
        })?;

        let buffer = RefCell::new(vec![]);
        let transport = Transport {
            web_socket,
            codec,
            state,
            buffer,
            _message_listener: message_listener,
            _error_listener: error_listener,
            _close_listener: close_listener,
            _outgoing: PhantomData,
        };
        Ok(transport)
    }

    /// Create new transport for WebSocket.
    ///
    /// It waits for WebSocket's `open` event before returning transport.
    pub async fn new(web_socket: &Rc<WebSocket>, codec: Codec) -> Result<Self, JsError> {
        let open_notifier = Rc::new(Queue::new());
        let open_notifier_clone = Rc::downgrade(&open_notifier);
        let _open_listener = web_socket.when("open", move |_event: Event| {
            if let Some(notifier) = open_notifier_clone.upgrade() {
                notifier.push(());
            } else {
                unreachable!("open message received twice!")
            }
        })?;

        let transport = Transport::new_assuming_open(web_socket, codec)?;

        open_notifier.pop().await;

        Ok(transport)
    }

    fn send_inner(
        &self,
        message: &Outgoing,
    ) -> Result<(), Error<<Codec as Encode>::Error, <Codec as Decode>::Error>> {
        let mut buffer = self.buffer.borrow_mut();
        self.codec
            .encode(&mut *buffer, &message)
            .map_err(Error::SerializationError)?;
        self.web_socket
            .send_with_u8_array(&buffer[..])
            .map_err(|error| Error::SendingError(error.into()))?;
        buffer.clear();
        Ok(())
    }
}

impl<Codec, Incoming, Outgoing> Sink<&Outgoing> for Transport<Codec, Incoming, Outgoing>
where
    Codec: 'static + kodec::Codec + Clone,
    Incoming: 'static,
    Outgoing: 'static + Serialize,
    <Codec as Encode>::Error: 'static,
    <Codec as Decode>::Error: 'static,
    for<'de> Incoming: serde::de::Deserialize<'de>,
{
    type Error = mezzenger::Error<Error<<Codec as Encode>::Error, <Codec as Decode>::Error>>;

    fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if self.state.borrow().closed {
            Poll::Ready(Err(mezzenger::Error::Closed))
        } else {
            Poll::Ready(Ok(()))
        }
    }

    fn start_send(self: Pin<&mut Self>, item: &Outgoing) -> Result<(), Self::Error> {
        if self.state.borrow().closed {
            Err(mezzenger::Error::Closed)
        } else {
            self.send_inner(item).map_err(mezzenger::Error::Other)
        }
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        if self.state.borrow().closed {
            Poll::Ready(Err(mezzenger::Error::Closed))
        } else {
            Poll::Ready(Ok(()))
        }
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if self.state.borrow().closed {
            Poll::Ready(Err(mezzenger::Error::Closed))
        } else {
            let result = self
                .web_socket
                .close()
                .map_err(|error| {
                    Error::<<Codec as Encode>::Error, <Codec as Decode>::Error>::ClosingError(
                        error.into(),
                    )
                })
                .map_err(mezzenger::Error::Other);
            self.state.borrow_mut().close();
            Poll::Ready(result)
        }
    }
}

impl<Codec, Incoming, Outgoing> Stream for Transport<Codec, Incoming, Outgoing>
where
    Codec: kodec::Codec,
{
    type Item = Result<Incoming, Error<<Codec as Encode>::Error, <Codec as Decode>::Error>>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut state = self.state.borrow_mut();
        if state.closed && state.incoming.is_empty() {
            Poll::Ready(None)
        } else if let Some(item) = state.incoming.pop_front() {
            Poll::Ready(Some(item))
        } else {
            state.update_waker_with(cx.waker());
            Poll::Pending
        }
    }
}

impl<Codec, Incoming, Outgoing> FusedStream for Transport<Codec, Incoming, Outgoing>
where
    Codec: kodec::Codec,
{
    fn is_terminated(&self) -> bool {
        let state = self.state.borrow();
        state.closed && state.incoming.is_empty()
    }
}

impl<Codec, Incoming, Outgoing> mezzenger::Reliable for Transport<Codec, Incoming, Outgoing> where
    Codec: kodec::Codec
{
}

impl<Codec, Incoming, Outgoing> mezzenger::Order for Transport<Codec, Incoming, Outgoing> where
    Codec: kodec::Codec
{
}
