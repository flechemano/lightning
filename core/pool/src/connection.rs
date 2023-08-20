use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use dashmap::DashMap;
use fleek_crypto::NodePublicKey;
use lightning_interfaces::schema::{AutoImplSerde, LightningMessage};
use lightning_interfaces::types::ServiceScope;
use quinn::{ClientConfig, Connection, Endpoint};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Receiver;

use crate::connector::ConnectEvent;
use crate::netkit;
use crate::pool::ScopeHandle;

pub async fn start_listener_driver(driver: ListenerDriver) {
    while let Some(connecting) = driver.endpoint.accept().await {
        let connection = connecting.await.unwrap();
        let handles = driver.handles.clone();
        tokio::spawn(async move {
            let (tx, mut rx) = connection.accept_bi().await.unwrap();
            let data = rx.read_to_end(4096).await.unwrap();
            let message: StreamRequest = StreamRequest::decode(&data).unwrap();
            if let Some(handle) = handles.get(&message.scope) {
                handle
                    .listener_tx
                    .send((message.source_peer, tx, rx))
                    .await
                    .unwrap();
            }
        });
    }
}

pub async fn start_connector_driver(mut driver: ConnectorDriver) {
    while let Some(event) = driver.connect_rx.recv().await {
        let connection = match driver.pool.get(&(event.pk, event.address)) {
            None => {
                let config = netkit::client_config();
                let client_config = ClientConfig::new(Arc::new(config));
                let connection = driver
                    .endpoint
                    .connect_with(client_config, event.address, "")
                    .unwrap()
                    .await
                    .unwrap();
                driver
                    .pool
                    .insert((event.pk, event.address), connection.clone());
                connection
            },
            Some(connection) => connection.clone(),
        };
        let (mut tx, rx) = connection.open_bi().await.unwrap();
        let mut writer = Vec::with_capacity(4096);

        LightningMessage::encode::<Vec<_>>(
            &StreamRequest {
                source_peer: event.pk,
                scope: event.scope,
            },
            writer.as_mut(),
        )
        .unwrap();
        let _ = tx.write(writer.as_mut()).await.unwrap();
        event.respond.send((tx, rx)).unwrap();
    }
}

/// Driver for driving the connection events from the transport connection.
pub struct ListenerDriver {
    /// Current active connections.
    handles: Arc<DashMap<ServiceScope, ScopeHandle>>,
    /// QUIC endpoint.
    endpoint: Endpoint,
}

impl ListenerDriver {
    pub fn new(handles: Arc<DashMap<ServiceScope, ScopeHandle>>, endpoint: Endpoint) -> Self {
        Self { handles, endpoint }
    }
}

/// Driver for driving the connection events from the transport connection.
pub struct ConnectorDriver {
    /// Listens for scoped service registration.
    connect_rx: Receiver<ConnectEvent>,
    /// QUIC connection pool.
    pool: HashMap<(NodePublicKey, SocketAddr), Connection>,
    /// QUIC endpoint.
    endpoint: Endpoint,
}

impl ConnectorDriver {
    pub fn new(connect_rx: Receiver<ConnectEvent>, endpoint: Endpoint) -> Self {
        Self {
            connect_rx,
            pool: HashMap::new(),
            endpoint,
        }
    }
}

/// Request use for establishing new stream connection.
#[derive(Deserialize, Serialize)]
pub struct StreamRequest {
    scope: ServiceScope,
    source_peer: NodePublicKey,
}

impl AutoImplSerde for StreamRequest {}
