use std::{cell::RefCell, marker::PhantomData, rc::Rc};

use js_sys::Uint8Array;
use js_utils::{
    event::{EventListener, When},
    JsError,
};
use kodec::{Decode, Encode};
use mezzenger_common::{
    rc::{Close, Receive, State},
    Send,
};
use serde::{Deserialize, Serialize};
use web_sys::{MessageEvent, Worker};

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
    Message(Message),
    Close,
}

pub struct Transport<Codec, Incoming, Outgoing>
where
    Codec: kodec::Codec,
{
    worker: Rc<Worker>,
    codec: Codec,
    state: Rc<RefCell<State<Incoming, Error<<Codec as Encode>::Error, <Codec as Decode>::Error>>>>,
    buffer: RefCell<Vec<u8>>,
    _message_listener: EventListener<Worker, MessageEvent>,
    _error_listener: EventListener<Worker, MessageEvent>,
    _message_error_listener: EventListener<Worker, MessageEvent>,
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
    pub fn new(worker: &Rc<Worker>, codec: Codec) -> Result<Self, JsError> {
        let worker = worker.clone();
        let codec_clone = codec.clone();
        let state = Rc::new(RefCell::new(State::new()));
        let state_clone = state.clone();
        let message_listener = worker.when("message", move |event: MessageEvent| {
            let array = Uint8Array::new(&event.data());
            let vector = array.to_vec();
            let result: Result<Wrapper<Incoming>, _> = codec_clone.decode(&vector[..]);
            match result {
                Ok(message) => match message {
                    Wrapper::Message(message) => state_clone.borrow_mut().message(message),
                    Wrapper::Close => state_clone.borrow_mut().close(),
                },
                Err(error) => state_clone
                    .borrow_mut()
                    .error(Error::DeserializationError(error)),
            }
        })?;
        let state_clone = state.clone();
        let error_listener = worker.when("error", move |event: MessageEvent| {
            state_clone.borrow_mut().error(Error::WorkerError(event));
        })?;
        let state_clone = state.clone();
        let message_error_listener = worker.when("messageerror", move |event: MessageEvent| {
            state_clone.borrow_mut().error(Error::MessageError(event));
        })?;
        let buffer = RefCell::new(vec![]);
        Ok(Transport {
            worker,
            codec,
            state,
            buffer,
            _message_listener: message_listener,
            _error_listener: error_listener,
            _message_error_listener: message_error_listener,
            _outgoing: PhantomData,
        })
    }

    fn send_inner(
        &self,
        message: Wrapper<&Outgoing>,
    ) -> Result<(), Error<<Codec as Encode>::Error, <Codec as Decode>::Error>> {
        let mut buffer = self.buffer.borrow_mut();
        self.codec
            .encode(&mut *buffer, &message)
            .map_err(|error| Error::SerializationError(error))?;
        let js_array = Uint8Array::from(&buffer[..]);
        self.worker
            .post_message(&js_array)
            .map_err(|error| Error::SendingError(error.into()))?;
        Ok(())
    }
}

pub struct Sender {}

impl<Codec, Incoming, Outgoing>
    mezzenger_common::Sender<
        Transport<Codec, Incoming, Outgoing>,
        Outgoing,
        Error<<Codec as Encode>::Error, <Codec as Decode>::Error>,
    > for Sender
where
    Codec: 'static + kodec::Codec + Clone,
    Incoming: 'static,
    Outgoing: 'static + Serialize,
    <Codec as Encode>::Error: 'static,
    <Codec as Decode>::Error: 'static,
    for<'de> Incoming: serde::de::Deserialize<'de>,
{
    fn send(
        transport: &Transport<Codec, Incoming, Outgoing>,
        message: &Outgoing,
    ) -> Result<(), mezzenger::Error<Error<<Codec as Encode>::Error, <Codec as Decode>::Error>>>
    {
        use mezzenger::Close;
        if transport.is_closed() {
            Err(mezzenger::Error::Closed)
        } else {
            transport
                .send_inner(Wrapper::Message(message))
                .map_err(|error| mezzenger::Error::Other(error))?;
            Ok(())
        }
    }
}

impl<Codec, Incoming, Outgoing> mezzenger::Send<Outgoing> for Transport<Codec, Incoming, Outgoing>
where
    Codec: 'static + kodec::Codec + Clone,
    Incoming: 'static,
    Outgoing: 'static + Serialize,
    <Codec as Encode>::Error: 'static,
    <Codec as Decode>::Error: 'static,
    for<'de> Incoming: serde::de::Deserialize<'de>,
{
    type Error = Error<<Codec as Encode>::Error, <Codec as Decode>::Error>;

    type Output<'a> = Send<'a, Transport<Codec, Incoming, Outgoing>, Outgoing, Error<<Codec as Encode>::Error, <Codec as Decode>::Error>, Sender>
    where
        Self: 'a;

    fn send<'s, 'm>(&'s self, message: &'m Outgoing) -> Self::Output<'s>
    where
        'm: 's,
    {
        Send::new(&self, message)
    }
}

impl<Codec, Incoming, Outgoing> mezzenger::Receive<Incoming>
    for Transport<Codec, Incoming, Outgoing>
where
    Codec: kodec::Codec,
{
    type Error = Error<<Codec as Encode>::Error, <Codec as Decode>::Error>;

    type Output<'a> = Receive<'a, Incoming, Error<<Codec as Encode>::Error, <Codec as Decode>::Error>>
    where
        Self: 'a;

    fn receive(&self) -> Self::Output<'_> {
        Receive::new(&self.state)
    }
}

pub struct Closer {}

impl<Codec, Incoming, Outgoing> mezzenger_common::rc::Closer<Transport<Codec, Incoming, Outgoing>>
    for Closer
where
    Codec: 'static + kodec::Codec + Clone,
    Incoming: 'static,
    Outgoing: 'static + Serialize,
    <Codec as Encode>::Error: 'static,
    <Codec as Decode>::Error: 'static,
    for<'de> Incoming: serde::de::Deserialize<'de>,
{
    fn close(transport: &Transport<Codec, Incoming, Outgoing>) {
        let _ = transport.send_inner(Wrapper::Close);
    }
}

impl<Codec, Incoming, Outgoing> mezzenger::Close for Transport<Codec, Incoming, Outgoing>
where
    Codec: 'static + kodec::Codec + Clone,
    Incoming: 'static,
    Outgoing: 'static + Serialize,
    <Codec as Encode>::Error: 'static,
    <Codec as Decode>::Error: 'static,
    for<'de> Incoming: serde::de::Deserialize<'de>,
{
    type Output<'a> = Close<'a, Transport<Codec, Incoming, Outgoing>, Incoming, Error<<Codec as Encode>::Error, <Codec as Decode>::Error>, Closer> where Self: 'a;

    fn close(&self) -> Self::Output<'_> {
        Close::new(&self, &self.state)
    }

    fn is_closed(&self) -> bool {
        self.state.borrow().closed
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
