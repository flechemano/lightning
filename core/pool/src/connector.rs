use std::marker::PhantomData;
use std::net::SocketAddr;

use async_trait::async_trait;
use fleek_crypto::NodePublicKey;
use lightning_interfaces::schema::LightningMessage;
use lightning_interfaces::types::ServiceScope;
use lightning_interfaces::{
    ConnectorInterface,
    SenderReceiver,
    SignerInterface,
    SyncQueryRunnerInterface,
};
use quinn::{Connection, RecvStream, SendStream};
use tokio::sync::{mpsc, oneshot};

use crate::pool::ConnectionPool;
use crate::receiver::Receiver;
use crate::sender::Sender;

pub struct ConnectEvent {
    pub scope: ServiceScope,
    pub pk: NodePublicKey,
    pub address: SocketAddr,
    pub respond: oneshot::Sender<(SendStream, RecvStream)>,
}

pub struct Connector<T> {
    scope: ServiceScope,
    connection_event_tx: mpsc::Sender<ConnectEvent>,
    _marker: PhantomData<T>,
}

impl<T> Connector<T> {
    pub fn new(scope: ServiceScope, connection_event_tx: mpsc::Sender<ConnectEvent>) -> Self {
        Self {
            scope,
            connection_event_tx,
            _marker: PhantomData::default(),
        }
    }
}

impl<T> Clone for Connector<T> {
    fn clone(&self) -> Self {
        todo!()
    }
}

#[async_trait]
impl<T> ConnectorInterface<T> for Connector<T>
where
    T: LightningMessage,
{
    type Sender = Sender<T>;
    type Receiver = Receiver<T>;
    async fn connect(&self, to: &NodePublicKey) -> Option<(Self::Sender, Self::Receiver)> {
        let (tx, rx) = oneshot::channel();
        self.connection_event_tx
            .send(ConnectEvent {
                scope: self.scope,
                pk: *to,
                address: "0.0.0.0:0".parse().unwrap(),
                respond: tx,
            })
            .await
            .ok()?;
        let (tx_stream, rx_stream) = rx.await.ok()?;
        Some((Sender::new(tx_stream, *to), Receiver::new(rx_stream, *to)))
    }
}