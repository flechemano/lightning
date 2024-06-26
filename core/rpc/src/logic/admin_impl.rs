use std::sync::Arc;

use jsonrpsee::core::RpcResult;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{Blake3Hash, CompressionAlgorithm};

use crate::api::AdminApiServer;
use crate::error::RPCError;
use crate::Data;

pub struct AdminApi<C: Collection> {
    data: Arc<Data<C>>,
}

impl<C: Collection> AdminApi<C> {
    pub(crate) fn new(data: Arc<Data<C>>) -> Self {
        Self { data }
    }
}

#[async_trait::async_trait]
impl<C: Collection> AdminApiServer for AdminApi<C> {
    async fn store(&self, path: String) -> RpcResult<Blake3Hash> {
        let file = tokio::fs::read(path)
            .await
            .map_err(|e| RPCError::custom(e.to_string()))?;

        let mut putter = self.data._blockstore.put(None);
        putter
            .write(file.as_ref(), CompressionAlgorithm::Uncompressed)
            .map_err(|e| RPCError::custom(format!("failed to write content: {e}")))?;
        let hash = putter
            .finalize()
            .await
            .map_err(|e| RPCError::custom(format!("failed to finalize put: {e}")))?;

        Ok(hash)
    }
}
