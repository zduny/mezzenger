//! Transport for communication with 
//! [Web Workers](https://developer.mozilla.org/en-US/docs/Web/API/Web_Workers_API).

use std::{
    cell::RefCell,
    collections::VecDeque,
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
use serde::{Deserialize, Serialize};
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{DedicatedWorkerGlobalScope, EventTarget, MessageEvent, Worker};

pub trait PostMessage {
    fn post_message(&self, message: &JsValue) -> Result<(), JsValue>;
}

impl PostMessage for Worker {
    fn post_message(&self, message: &JsValue) -> Result<(), JsValue> {
        self.post_message(message)
    }
}

impl PostMessage for DedicatedWorkerGlobalScope {
    fn post_message(&self, message: &JsValue) -> Result<(), JsValue> {
        self.post_message(message)
    }
}

#[derive(Debug)]
pub enum Error<SerializationError, DeserializationError> {
    SendingError(JsError),
    SerializationError(SerializationError),
    DeserializationError(DeserializationError),
    WorkerError(MessageEvent),
    MessageError(MessageEvent),
}

#[derive(Debug, Serialize, Deserialize)]
enum Wrapper<Message> {
    Open,
    Message(Message),
    Close,
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

/// Transport for communication with 
/// [Web Workers](https://developer.mozilla.org/en-US/docs/Web/API/Web_Workers_API).
pub struct Transport<T, Codec, Incoming, Outgoing>
where
    T: AsRef<EventTarget> + PostMessage,
    Codec: kodec::Codec,
{
    target: Rc<T>,
    codec: Codec,
    #[allow(clippy::type_complexity)]
    state: Rc<RefCell<State<Incoming, Error<<Codec as Encode>::Error, <Codec as Decode>::Error>>>>,
    buffer: RefCell<Vec<u8>>,
    _message_listener: EventListener<T, MessageEvent>,
    _error_listener: EventListener<T, MessageEvent>,
    _message_error_listener: EventListener<T, MessageEvent>,
    _outgoing: PhantomData<Outgoing>,
}

impl<T, Codec, Incoming, Outgoing> Transport<T, Codec, Incoming, Outgoing>
where
    T: AsRef<EventTarget> + PostMessage,
    Codec: 'static + kodec::Codec + Clone,
    Incoming: 'static,
    Outgoing: 'static + Serialize,
    <Codec as Encode>::Error: 'static,
    <Codec as Decode>::Error: 'static,
    for<'de> Incoming: serde::de::Deserialize<'de>,
{
    async fn new_inner(target: &Rc<T>, codec: Codec, is_worker: bool) -> Result<Self, JsError> {
        let open_notifier = Rc::new(Queue::new());

        let target = target.clone();
        let codec_clone = codec.clone();
        let state = Rc::new(RefCell::new(State::new()));
        let state_clone = state.clone();
        let open_notifier_clone = Rc::downgrade(&open_notifier);
        let message_listener = target.when("message", move |event: MessageEvent| {
            let array = Uint8Array::new(&event.data());
            let vector = array.to_vec();
            let result: Result<Wrapper<Incoming>, _> = codec_clone.decode(&vector[..]);
            match result {
                Ok(message) => match message {
                    Wrapper::Open => {
                        if let Some(notifier) = open_notifier_clone.upgrade() {
                            notifier.push(());
                        } else {
                            unreachable!("open message received twice!")
                        }
                    }
                    Wrapper::Message(message) => state_clone.borrow_mut().message(message),
                    Wrapper::Close => state_clone.borrow_mut().close(),
                },
                Err(error) => state_clone
                    .borrow_mut()
                    .error(Error::DeserializationError(error)),
            }
        })?;
        let state_clone = state.clone();
        let error_listener = target.when("error", move |event: MessageEvent| {
            state_clone.borrow_mut().error(Error::WorkerError(event));
        })?;
        let state_clone = state.clone();
        let message_error_listener = target.when("messageerror", move |event: MessageEvent| {
            state_clone.borrow_mut().error(Error::MessageError(event));
        })?;
        let buffer = RefCell::new(vec![]);
        let transport = Transport {
            target,
            codec,
            state,
            buffer,
            _message_listener: message_listener,
            _error_listener: error_listener,
            _message_error_listener: message_error_listener,
            _outgoing: PhantomData,
        };

        if is_worker {
            let _ = transport.send_inner(Wrapper::Open);
            open_notifier.pop().await;
        } else {
            open_notifier.pop().await;
            let _ = transport.send_inner(Wrapper::Open);
        }

        Ok(transport)
    }

    fn send_inner(
        &self,
        message: Wrapper<&Outgoing>,
    ) -> Result<(), Error<<Codec as Encode>::Error, <Codec as Decode>::Error>> {
        let mut buffer = self.buffer.borrow_mut();
        self.codec
            .encode(&mut *buffer, &message)
            .map_err(Error::SerializationError)?;
        let js_array = Uint8Array::from(&buffer[..]);
        self.target
            .post_message(&js_array)
            .map_err(|error| Error::SendingError(error.into()))?;
        buffer.clear();
        Ok(())
    }
}

impl<Codec, Incoming, Outgoing> Transport<Worker, Codec, Incoming, Outgoing>
where
    Codec: 'static + kodec::Codec + Clone,
    Incoming: 'static,
    Outgoing: 'static + Serialize,
    <Codec as Encode>::Error: 'static,
    <Codec as Decode>::Error: 'static,
    for<'de> Incoming: serde::de::Deserialize<'de>,
{
    /// Create new transport for communication with worker.
    pub async fn new(worker: &Rc<Worker>, codec: Codec) -> Result<Self, JsError> {
        Transport::new_inner(worker, codec, false).await
    }
}

impl<Codec, Incoming, Outgoing> Transport<DedicatedWorkerGlobalScope, Codec, Incoming, Outgoing>
where
    Codec: 'static + kodec::Codec + Clone,
    Incoming: 'static,
    Outgoing: 'static + Serialize,
    <Codec as Encode>::Error: 'static,
    <Codec as Decode>::Error: 'static,
    for<'de> Incoming: serde::de::Deserialize<'de>,
{
    /// Create new transport inside worker.
    ///
    /// Will panic if called outside worker scope.
    pub async fn new_in_worker(codec: Codec) -> Result<Self, JsError> {
        let global = Rc::new(
            js_sys::global()
                .dyn_into::<DedicatedWorkerGlobalScope>()
                .unwrap(),
        );
        Transport::new_inner(&global, codec, true).await
    }
}

impl<T, Codec, Incoming, Outgoing> Sink<&Outgoing> for Transport<T, Codec, Incoming, Outgoing>
where
    T: AsRef<EventTarget> + PostMessage,
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
            self.send_inner(Wrapper::Message(item))
                .map_err(mezzenger::Error::Other)
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
            let _ = self.send_inner(Wrapper::Close);
            self.state.borrow_mut().close();
            Poll::Ready(Ok(()))
        }
    }
}

impl<T, Codec, Incoming, Outgoing> Stream for Transport<T, Codec, Incoming, Outgoing>
where
    T: AsRef<EventTarget> + PostMessage,
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

impl<T, Codec, Incoming, Outgoing> FusedStream for Transport<T, Codec, Incoming, Outgoing>
where
    T: AsRef<EventTarget> + PostMessage,
    Codec: kodec::Codec,
{
    fn is_terminated(&self) -> bool {
        let state = self.state.borrow();
        state.closed && state.incoming.is_empty()
    }
}

impl<T, Codec, Incoming, Outgoing> mezzenger::Reliable for Transport<T, Codec, Incoming, Outgoing>
where
    T: AsRef<EventTarget> + PostMessage,
    Codec: kodec::Codec,
{
}

impl<T, Codec, Incoming, Outgoing> mezzenger::Order for Transport<T, Codec, Incoming, Outgoing>
where
    T: AsRef<EventTarget> + PostMessage,
    Codec: kodec::Codec,
{
}
