use affair::AsyncWorkerUnordered;
use lightning_interfaces::types::{Blake3Hash, ImmutablePointer, OriginProvider};
use lightning_interfaces::Collection;
use lightning_origin_http::HttpOrigin;
use lightning_origin_ipfs::IPFSOrigin;

use crate::Config;

pub struct Demuxer<C: Collection> {
    http: HttpOrigin<C>,
    ipfs: IPFSOrigin<C>,
}

impl<C: Collection> AsyncWorkerUnordered for Demuxer<C> {
    type Request = ImmutablePointer;
    type Response = anyhow::Result<Blake3Hash>;

    async fn handle(&self, req: Self::Request) -> Self::Response {
        match &req.origin {
            OriginProvider::HTTP => self.http.fetch(&req.uri).await,
            OriginProvider::IPFS => self.ipfs.fetch(&req.uri).await,
            _ => Err(anyhow::anyhow!("unknown origin type")),
        }
    }
}

impl<C: Collection> Demuxer<C> {
    pub fn new(config: Config, blockstore: C::BlockstoreInterface) -> anyhow::Result<Self> {
        Ok(Self {
            http: HttpOrigin::<C>::new(config.http, blockstore.clone())?,
            ipfs: IPFSOrigin::<C>::new(config.ipfs, blockstore)?,
        })
    }
}
