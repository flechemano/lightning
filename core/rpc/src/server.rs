use std::{
    net::SocketAddr,
    sync::{Arc, RwLock},
};

use async_trait::async_trait;
use axum::{
    routing::{get, post},
    Extension, Router,
};
use draco_interfaces::{
    common::WithStartAndShutdown, config::ConfigConsumer, MempoolSocket, RpcInterface,
    SyncQueryRunnerInterface,
};

use super::config::Config;
use crate::handlers::{rpc_handler, RpcServer};

#[derive(Clone)]
pub struct Rpc<Q: SyncQueryRunnerInterface> {
    _mempool_address: MempoolSocket,
    query_runner: Q,
    server_running: Arc<RwLock<bool>>,
    pub config: Config,
}

impl<Q: SyncQueryRunnerInterface> Rpc<Q> {
    fn set_running(&self, status: bool) {
        if let Ok(mut server_running) = self.server_running.write() {
            *server_running = status;
        }
    }
}

#[async_trait]
impl<Q: SyncQueryRunnerInterface + 'static> WithStartAndShutdown for Rpc<Q> {
    /// Returns true if this system is running or not.
    fn is_running(&self) -> bool {
        *self.server_running.read().unwrap()
    }

    /// Start the system, should not do anything if the system is already
    /// started.
    async fn start(&self) {
        if !self.is_running() {
            println!("RPC server starting up");
            let rpc = Arc::new(self.clone());
            let server = RpcServer::new(Arc::clone(&rpc));

            let app = Router::new()
                .route("/health", get(|| async { "OK" }))
                .route("/rpc/v0", post(rpc_handler))
                .layer(Extension(server.clone()));

            self.set_running(true);
            let http_address = SocketAddr::from(([127, 0, 0, 1], self.config.port));
            println!("listening on {http_address}");
            axum::Server::bind(&http_address)
                .serve(app.into_make_service())
                .await
                .expect("Server should not fail to start");
        }
    }

    /// Send the shutdown signal to the system.
    async fn shutdown(&self) {
        self.set_running(false);
        // more loggic here
        todo!()
    }
}

#[async_trait]
impl<Q: SyncQueryRunnerInterface + Send + Sync + 'static> RpcInterface<Q> for Rpc<Q> {
    /// Initialize the *RPC* server, with the given parameters.
    async fn init(
        config: Self::Config,
        mempool: MempoolSocket,
        query_runner: Q,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            _mempool_address: mempool,
            query_runner,
            config,
            server_running: Arc::new(RwLock::new(false)),
        })
    }
    fn query_runner(&self) -> Q {
        self.query_runner.clone()
    }
}

impl<Q: SyncQueryRunnerInterface> ConfigConsumer for Rpc<Q> {
    const KEY: &'static str = "rpc";

    type Config = Config;
}
