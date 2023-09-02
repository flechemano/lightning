pub mod config;

use std::io::{Read, Write};
use std::marker::PhantomData;
use std::net::{SocketAddr, TcpStream};
use std::sync::RwLock;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use blake3_stream::{Encoder, FrameDecoder};
use blake3_tree::blake3::tree::HashTree;
use config::Config;
use lightning_interfaces::blockstore_server::BlockStoreServerInterface;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::{CompressionAlgoSet, CompressionAlgorithm, NodeIndex};
use lightning_interfaces::{
    Blake3Hash,
    BlockStoreInterface,
    ConfigConsumer,
    IncrementalPutInterface,
    SyncQueryRunnerInterface,
    WithStartAndShutdown,
};
use log::error;
use tokio::net::TcpListener;
use tokio::select;
use triomphe::Arc;

pub struct BlockStoreServer<C: Collection> {
    phantom: PhantomData<C>,
    config: Arc<Config>,
    blockstore: C::BlockStoreInterface,
    shutdown_tx: Arc<RwLock<Option<tokio::sync::oneshot::Sender<()>>>>,
}

impl<C: Collection> Clone for BlockStoreServer<C> {
    fn clone(&self) -> Self {
        Self {
            phantom: self.phantom,
            config: self.config.clone(),
            blockstore: self.blockstore.clone(),
            shutdown_tx: self.shutdown_tx.clone(),
        }
    }
}

impl<C: Collection> ConfigConsumer for BlockStoreServer<C> {
    const KEY: &'static str = "blockserver";
    type Config = Config;
}

#[async_trait]
impl<C: Collection> WithStartAndShutdown for BlockStoreServer<C> {
    fn is_running(&self) -> bool {
        self.shutdown_tx.read().unwrap().is_some()
    }

    /// Start the system, should not do anything if the system is already
    /// started.
    async fn start(&self) {
        if self.shutdown_tx.read().unwrap().is_some() {
            return;
        }

        // spawn server task
        let address = self.config.address;
        let blockstore = self.blockstore.clone();

        // bind to address
        let listener = TcpListener::bind(address)
            .await
            .expect("failed to bind to address");

        let (tx, mut rx) = tokio::sync::oneshot::channel();
        *self.shutdown_tx.write().unwrap() = Some(tx);

        tokio::spawn(async move {
            loop {
                select! {
                    Ok((socket, _)) = listener.accept() => {
                        let blockstore = blockstore.clone();
                        tokio::spawn(async move {
                            let socket = socket.into_std().unwrap();
                            if let Err(e) = handle_connection::<C>(blockstore, socket).await {
                                error!("error handling blockstore connection: {e}");
                            }
                        });
                    },
                    _ = &mut rx => break,
                }
            }
        });
    }

    /// Send the shutdown signal to the system.
    async fn shutdown(&self) {
        let sender = self.shutdown_tx.write().unwrap().take().unwrap();
        sender.send(()).unwrap();
    }
}

async fn handle_connection<C: Collection>(
    blockstore: C::BlockStoreInterface,
    mut socket: TcpStream,
) -> anyhow::Result<()> {
    let mut hash = [0u8; 32];
    socket.read_exact(&mut hash)?;

    // fetch from the blockstore
    let Some(proof) = blockstore.get_tree(&hash).await else {
        return Err(anyhow!("failed to get proof"));
    };

    // find out total content size
    let mut last_hash = [0; 32];
    let mut total = 0;
    for i in 0u32.. {
        let ii = (i * 2 - i.count_ones()) as usize;
        if ii >= proof.0.len() {
            break;
        }
        last_hash = proof.0[ii];
        total += 1;
    }

    let content_len = blockstore
        .get(total - 1, &last_hash, CompressionAlgoSet::default())
        .await
        .expect("last block not available")
        .content
        .len()
        + (total as usize - 1) * 256 * 1024;

    // Setup stream encoder

    let mut encoder = Encoder::new(
        socket,
        content_len,
        HashTree {
            hash: hash.into(),
            tree: proof.0.clone(),
        },
    )?;

    // Feed blocks to the stream
    let mut block_counter = 0u32;
    loop {
        let idx = (block_counter * 2 - block_counter.count_ones()) as usize;
        if idx >= proof.0.len() {
            break;
        }

        let block = blockstore
            .get(block_counter, &proof.0[idx], CompressionAlgoSet::default())
            .await
            .ok_or(anyhow!("failed to get block"))?;
        encoder.write_all(&block.content)?;

        block_counter += 1;
    }

    Ok(())
}

#[async_trait]
impl<C: Collection> BlockStoreServerInterface<C> for BlockStoreServer<C> {
    fn init(config: Self::Config, blockstore: C::BlockStoreInterface) -> anyhow::Result<Self> {
        Ok(Self {
            phantom: PhantomData,
            config: config.into(),
            blockstore,
            shutdown_tx: Arc::new(RwLock::new(None)),
        })
    }

    fn extract_address<Q: SyncQueryRunnerInterface>(
        query_runner: Q,
        target: NodeIndex,
    ) -> Option<SocketAddr> {
        // Get node pk, info, and finally the address
        query_runner.index_to_pubkey(target).and_then(|pk| {
            query_runner
                .get_node_info(&pk)
                .map(|info| (info.domain, info.ports.blockstore).into())
        })
    }

    async fn request_download(&self, block_hash: Blake3Hash, target: SocketAddr) -> Result<()> {
        // Connect to the destination
        let mut socket = TcpStream::connect(target)?;

        // Send request
        socket.write_all(&block_hash)?;

        // Setup the decoder
        let mut decoder = FrameDecoder::new(socket);

        let mut putter = self.blockstore.put(Some(block_hash));
        while let Some(frame) = decoder.next_frame()? {
            match frame {
                blake3_stream::FrameBytes::Proof(bytes) => {
                    putter.feed_proof(&bytes)?;
                },
                blake3_stream::FrameBytes::Chunk(bytes) => {
                    putter.write(&bytes, CompressionAlgorithm::Uncompressed)?;
                },
            }
        }

        let hash = putter.finalize().await?;
        debug_assert_eq!(hash, block_hash);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use lightning_blockstore::blockstore::Blockstore;
    use lightning_interfaces::infu_collection::Collection;
    use lightning_interfaces::partial;

    use super::*;

    partial!(TestBindings {
        BlockStoreInterface = Blockstore<Self>;
        BlockStoreServerInterface = BlockStoreServer<Self>;
    });

    // tests need to be run with multi threaded, otherwise the server spawn is never polled.
    #[tokio::test(flavor = "multi_thread")]
    async fn request_download() -> Result<()> {
        // Setup two servers
        let blockstore_a =
            Blockstore::<TestBindings>::init(lightning_blockstore::config::Config::default())?;
        let address = "0.0.0.0:17000".parse().unwrap();
        let server_a =
            BlockStoreServer::<TestBindings>::init(Config { address }, blockstore_a.clone())?;
        server_a.start().await;

        // load some content into the first blockstore
        let mut putter = blockstore_a.put(None);
        // TODO: test different content sizes once blockstore is debugged (failing for any odd
        // number of blocks)
        putter.write(&[0u8; 2 * 256 * 1024], CompressionAlgorithm::Uncompressed)?;
        let hash = putter.finalize().await?;

        let blockstore_b =
            Blockstore::<TestBindings>::init(lightning_blockstore::config::Config::default())?;
        let server_b = BlockStoreServer::<TestBindings>::init(
            Config {
                address: "127.0.0.1:17001".parse().unwrap(),
            },
            blockstore_b.clone(),
        )?;

        // Request download from server a, loading the content into b
        server_b
            .request_download(hash, "127.0.0.1:17000".parse().unwrap())
            .await?;

        // Verify blockstore b has the fetched content
        assert!(blockstore_b.get_tree(&hash).await.is_some());

        server_a.shutdown().await;
        Ok(())
    }
}
