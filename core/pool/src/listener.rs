use std::marker::PhantomData;
use std::sync::Mutex;

use async_trait::async_trait;
use fleek_crypto::NodePublicKey;
use lightning_interfaces::schema::LightningMessage;
use lightning_interfaces::{
    ListenerInterface,
    SenderReceiver,
    SignerInterface,
    SyncQueryRunnerInterface,
};
use quinn::{Connection, ConnectionError, Endpoint, RecvStream, SendStream};
use tokio::sync::mpsc;

use crate::connection::RegisterEvent;
use crate::pool::ConnectionPool;
use crate::receiver::Receiver;
use crate::sender::Sender;

pub struct Listener<T> {
    registered: bool,
    register_tx: mpsc::Sender<RegisterEvent>,
    connection_event_tx: mpsc::Sender<Option<(NodePublicKey, SendStream, RecvStream)>>,
    connection_event_rx: mpsc::Receiver<Option<(NodePublicKey, SendStream, RecvStream)>>,
    _marker: PhantomData<T>,
}

impl<T> Listener<T> {
    pub fn new(
        registered: bool,
        register_tx: mpsc::Sender<RegisterEvent>,
        connection_event_tx: mpsc::Sender<Option<(NodePublicKey, SendStream, RecvStream)>>,
        connection_event_rx: mpsc::Receiver<Option<(NodePublicKey, SendStream, RecvStream)>>,
    ) -> Self {
        Self {
            registered,
            register_tx,
            connection_event_tx,
            connection_event_rx,
            _marker: PhantomData::default(),
        }
    }
}

#[async_trait]
impl<T> ListenerInterface<T> for Listener<T>
where
    T: LightningMessage,
{
    type Sender = Sender<T>;
    type Receiver = Receiver<T>;
    async fn accept(&mut self) -> Option<(Self::Sender, Self::Receiver)> {
        let (peer, tx, rx) = self.connection_event_rx.recv().await.flatten()?;
        Some((Sender::new(tx, peer), Receiver::new(rx, peer)))
    }
}