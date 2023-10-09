use std::marker::PhantomData;
use std::sync::Mutex;

use anyhow::Result;
use async_trait::async_trait;
use lightning_interfaces::infu_collection::{c, Collection};
use lightning_interfaces::types::Blake3Hash;
use lightning_interfaces::{
    ApplicationInterface,
    Notification,
    SyncronizerInterface,
    WithStartAndShutdown,
};
use tokio::sync::mpsc::Receiver;
use tokio::sync::oneshot;

pub struct MockSyncronizer<C: Collection> {
    rx_checkpoint_ready: Mutex<Option<oneshot::Receiver<Blake3Hash>>>,
    _tx_checkpoint_ready: oneshot::Sender<Blake3Hash>,
    _marker: PhantomData<C>,
}

#[async_trait]
impl<C: Collection> WithStartAndShutdown for MockSyncronizer<C> {
    /// Returns true if this system is running or not.
    fn is_running(&self) -> bool {
        false
    }

    /// Start the system, should not do anything if the system is already
    /// started.
    async fn start(&self) {}

    /// Send the shutdown signal to the system.
    async fn shutdown(&self) {}
}

impl<C: Collection> SyncronizerInterface<C> for MockSyncronizer<C> {
    fn init(
        _query_runner: c!(C::ApplicationInterface::SyncExecutor),
        _blockstore_server: &C::BlockStoreServerInterface,
        _rx_epoch_change: Receiver<Notification>,
    ) -> Result<Self> {
        let (tx, rx) = oneshot::channel();
        Ok(Self {
            rx_checkpoint_ready: Mutex::new(Some(rx)),
            _tx_checkpoint_ready: tx,
            _marker: PhantomData,
        })
    }

    fn checkpoint_socket(&self) -> oneshot::Receiver<Blake3Hash> {
        self.rx_checkpoint_ready.lock().unwrap().take().unwrap()
    }
}