use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::Duration;

use fleek_crypto::{
    AccountOwnerSecretKey,
    ConsensusPublicKey,
    EthAddress,
    NodePublicKey,
    SecretKey,
};
use futures::future::try_join_all;
use hp_fixed::unsigned::HpUfixed;
use lightning_application::app::Application;
use lightning_application::config::{Config as AppConfig, Mode, StorageConfig};
use lightning_application::genesis::{Genesis, GenesisNode};
use lightning_archive::archive::Archive;
use lightning_archive::config::Config as ArchiveConfig;
use lightning_blockstore::blockstore::Blockstore;
use lightning_blockstore::config::Config as BlockstoreConfig;
use lightning_blockstore_server::{BlockstoreServer, Config as BlockstoreServerConfig};
use lightning_consensus::config::Config as ConsensusConfig;
use lightning_consensus::consensus::Consensus;
use lightning_final_bindings::FinalTypes;
use lightning_handshake::config::{HandshakeConfig, TransportConfig};
use lightning_handshake::handshake::Handshake;
use lightning_handshake::transports::webrtc::WebRtcConfig;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{NodePorts, Staking};
use lightning_keystore::{Keystore, KeystoreConfig};
use lightning_pinger::{Config as PingerConfig, Pinger};
use lightning_pool::{Config as PoolConfig, PoolProvider};
use lightning_rep_collector::config::Config as RepAggConfig;
use lightning_rep_collector::ReputationAggregator;
use lightning_resolver::config::Config as ResolverConfig;
use lightning_resolver::resolver::Resolver;
use lightning_rpc::config::Config as RpcConfig;
use lightning_rpc::Rpc;
use lightning_service_executor::shim::{ServiceExecutor, ServiceExecutorConfig};
use lightning_syncronizer::config::Config as SyncronizerConfig;
use lightning_syncronizer::syncronizer::Syncronizer;
use lightning_utils::config::TomlConfigProvider;
use resolved_pathbuf::ResolvedPathBuf;

use crate::containerized_node::ContainerizedNode;
use crate::utils::networking::{PortAssigner, Transport};

pub struct Swarm {
    nodes: HashMap<NodePublicKey, ContainerizedNode>,
    directory: ResolvedPathBuf,
}

impl Drop for Swarm {
    fn drop(&mut self) {
        self.shutdown_internal();
    }
}

impl Swarm {
    pub fn builder() -> SwarmBuilder {
        SwarmBuilder::default()
    }

    pub async fn launch(&self) -> anyhow::Result<()> {
        try_join_all(self.nodes.values().map(|node| node.start())).await?;
        Ok(())
    }

    pub async fn launch_genesis_committee(&self) -> anyhow::Result<()> {
        try_join_all(
            self.nodes
                .values()
                .filter(|node| node.is_genesis_committee())
                .map(|node| node.start()),
        )
        .await?;
        Ok(())
    }

    pub async fn launch_non_genesis_committee(&self) -> anyhow::Result<()> {
        try_join_all(
            self.nodes
                .values()
                .filter(|node| !node.is_genesis_committee())
                .map(|node| node.start()),
        )
        .await?;
        Ok(())
    }

    pub fn shutdown(mut self) {
        self.shutdown_internal();
    }

    pub fn get_rpc_addresses(&self) -> HashMap<NodePublicKey, String> {
        self.nodes
            .iter()
            .map(|(pubkey, node)| (*pubkey, node.get_rpc_address()))
            .collect()
    }

    pub fn get_genesis_stakes(&self) -> HashMap<NodePublicKey, Staking> {
        self.nodes
            .iter()
            .map(|(pubkey, node)| (*pubkey, node.get_genesis_stake()))
            .collect()
    }

    pub fn get_genesis_committee_rpc_addresses(&self) -> HashMap<NodePublicKey, String> {
        self.nodes
            .iter()
            .filter(|(_, node)| node.is_genesis_committee())
            .map(|(pubkey, node)| (*pubkey, node.get_rpc_address()))
            .collect()
    }

    pub fn get_non_genesis_committee_rpc_addresses(&self) -> HashMap<NodePublicKey, String> {
        self.nodes
            .iter()
            .filter(|(_, node)| !node.is_genesis_committee())
            .map(|(pubkey, node)| (*pubkey, node.get_rpc_address()))
            .collect()
    }

    pub fn get_non_genesis_committee_syncronizer(
        &self,
    ) -> Vec<(
        NodePublicKey,
        Option<fdi::Ref<c!(FinalTypes::SyncronizerInterface)>>,
    )> {
        self.nodes
            .iter()
            .filter(|(_pubkey, node)| !node.is_genesis_committee())
            .map(|(pubkey, node)| (*pubkey, node.take_syncronizer()))
            .collect()
    }

    pub fn get_blockstores(&self) -> Vec<Option<Blockstore<FinalTypes>>> {
        self.nodes
            .values()
            .map(|node| node.take_blockstore())
            .collect()
    }

    pub fn get_blockstore(&self, node: &NodePublicKey) -> Option<Blockstore<FinalTypes>> {
        self.nodes.get(node).and_then(|node| node.take_blockstore())
    }

    fn shutdown_internal(&mut self) {
        self.nodes.values().for_each(|node| node.shutdown());
        if self.directory.exists() {
            fs::remove_dir_all(&self.directory).expect("Failed to clean up swarm directory.");
        }
    }
}

#[derive(Default)]
pub struct SwarmBuilder {
    directory: Option<ResolvedPathBuf>,
    min_port: Option<u16>,
    max_port: Option<u16>,
    num_nodes: Option<usize>,
    node_count_param: Option<u64>,
    epoch_start: Option<u64>,
    epoch_time: Option<u64>,
    port_assigner: Option<PortAssigner>,
    syncronizer_delta: Option<Duration>,
    archiver: bool,
    use_persistence: bool,
    specific_nodes: Option<Vec<SwarmNode>>,
    committee_size: Option<u64>,
}

impl SwarmBuilder {
    pub fn with_directory(mut self, directory: ResolvedPathBuf) -> Self {
        self.directory = Some(directory);
        self
    }

    pub fn with_num_nodes(mut self, num_nodes: usize) -> Self {
        self.num_nodes = Some(num_nodes);
        self
    }

    /// Sets the protocol param node_count on the swarms genesis
    pub fn with_node_count_param(mut self, num_nodes: u64) -> Self {
        self.node_count_param = Some(num_nodes);
        self
    }

    pub fn with_epoch_start(mut self, epoch_start: u64) -> Self {
        self.epoch_start = Some(epoch_start);
        self
    }

    pub fn with_epoch_time(mut self, epoch_time: u64) -> Self {
        self.epoch_time = Some(epoch_time);
        self
    }

    pub fn with_port_assigner(mut self, port_assigner: PortAssigner) -> Self {
        self.port_assigner = Some(port_assigner);
        self
    }

    pub fn persistence(mut self, persistence: bool) -> Self {
        self.use_persistence = persistence;
        self
    }

    pub fn with_committee_size(mut self, committee_size: u64) -> Self {
        self.committee_size = Some(committee_size);
        self
    }

    pub fn with_min_port(mut self, port: u16) -> Self {
        self.min_port = Some(port);
        self
    }

    pub fn with_max_port(mut self, port: u16) -> Self {
        self.max_port = Some(port);
        self
    }

    pub fn with_archiver(mut self) -> Self {
        self.archiver = true;
        self
    }

    pub fn with_syncronizer_delta(mut self, delta: Duration) -> Self {
        self.syncronizer_delta = Some(delta);
        self
    }

    pub fn with_specific_nodes(mut self, nodes: Vec<SwarmNode>) -> Self {
        self.specific_nodes = Some(nodes);
        self
    }

    pub fn build(self) -> Swarm {
        let num_nodes = self.num_nodes.expect("Number of nodes must be provided.");
        let directory = self.directory.expect("Directory must be provided.");
        let min_port = self.min_port.expect("Minimum port must be provided.");
        let max_port = self.max_port.unwrap_or(min_port + 100);

        let mut port_assigner = self
            .port_assigner
            .unwrap_or_else(|| PortAssigner::new(min_port, max_port));

        // Load the default genesis. Clear the committee and node info and overwrite
        // the provided values from config.
        let mut genesis = Genesis::load().unwrap();

        genesis.node_info = Vec::with_capacity(num_nodes);
        genesis.epoch_start = self.epoch_start.unwrap_or(genesis.epoch_start);
        genesis.epoch_time = self.epoch_time.unwrap_or(genesis.epoch_time);
        genesis.committee_size = self.committee_size.unwrap_or(genesis.committee_size);
        genesis.node_count = self.node_count_param.unwrap_or(genesis.node_count);

        // Make sure the test directory exists by recursively creating it.
        fs::create_dir_all(&directory).expect("Failed to create swarm directory");

        // For the number of nodes that we need. Create the distinct configuration objects which
        // we can pass to the containerized nodes.
        let mut tmp_nodes = Vec::with_capacity(num_nodes);

        let mut index = 0;
        let mut committee_size = 0;

        let mut specific_nodes = self.specific_nodes.unwrap_or_default();
        if specific_nodes.len() > num_nodes {
            panic!(
                "Number of nodes is {num_nodes}, but {} additional nodes were specified.",
                specific_nodes.len()
            );
        }

        while index < num_nodes {
            let node = specific_nodes.pop();

            let stake = node.clone().and_then(|node| node.stake).unwrap_or(Staking {
                staked: HpUfixed::<18>::from(genesis.min_stake + 1000_u64),
                ..Default::default()
            });
            let reputation_score = node.clone().and_then(|node| node.reputation_score);
            let is_committee = node.map(|node| node.is_committee).unwrap_or(false);
            if committee_size == genesis.committee_size && is_committee {
                panic!(
                    "Committee size is set to {}. Too many additional nodes that are on the committee were specified.",
                    genesis.committee_size
                )
            }
            let is_committee = is_committee || committee_size < genesis.committee_size;
            if is_committee {
                committee_size += 1;
            }

            let root = directory.join(format!("node-{index}"));
            fs::create_dir_all(&root).expect("Failed to create node directory");

            let ports = assign_ports(&mut port_assigner);
            let config = build_config(
                &root,
                ports.clone(),
                self.archiver,
                self.syncronizer_delta.unwrap_or(Duration::from_secs(300)),
            );

            // Generate and store the node public key.
            let (node_pk, consensus_pk) = generate_and_store_node_secret(&config);
            let owner_sk = AccountOwnerSecretKey::generate();
            let owner_pk = owner_sk.to_pk();
            let owner_eth: EthAddress = owner_pk.into();

            let is_committee = (index as u64) < genesis.committee_size;

            let node_info = GenesisNode {
                owner: owner_eth,
                primary_public_key: node_pk,
                consensus_public_key: consensus_pk,
                primary_domain: "127.0.0.1".parse().unwrap(),
                worker_domain: "127.0.0.1".parse().unwrap(),
                worker_public_key: node_pk,
                ports,
                stake: stake.clone(),
                reputation: reputation_score,
                current_epoch_served: None,
                genesis_committee: is_committee,
            };

            genesis.node_info.push(node_info);

            tmp_nodes.push((owner_sk, node_pk, config, is_committee, stake));

            index += 1;
        }

        // Now that we have built the configuration of all nodes and also have compiled the
        // proper genesis config. We can inject the genesis config.
        let mut nodes = HashMap::new();
        for (index, (owner_sk, node_pk, config, is_committee, stake)) in
            tmp_nodes.into_iter().enumerate()
        {
            let root = directory.join(format!("node-{index}"));
            let storage = if self.use_persistence {
                StorageConfig::RocksDb
            } else {
                StorageConfig::InMemory
            };
            config.inject::<Application<FinalTypes>>(AppConfig {
                mode: Mode::Test,
                genesis: Some(genesis.clone()),
                testnet: false,
                storage,
                db_path: Some(root.join("data/app_db").try_into().unwrap()),
                db_options: None,
            });

            let node = ContainerizedNode::new(config, owner_sk, index, is_committee, stake);
            nodes.insert(node_pk, node);
        }

        Swarm { nodes, directory }
    }
}

fn assign_ports(port_assigner: &mut PortAssigner) -> NodePorts {
    NodePorts {
        primary: port_assigner
            .next_port(Transport::Udp)
            .expect("Could not get port"),
        worker: port_assigner
            .next_port(Transport::Udp)
            .expect("Could not get port"),
        mempool: port_assigner
            .next_port(Transport::Tcp)
            .expect("Could not get port"),
        rpc: port_assigner
            .next_port(Transport::Tcp)
            .expect("Could not get port"),
        pool: port_assigner
            .next_port(Transport::Udp)
            .expect("Could not get port"),
        pinger: port_assigner
            .next_port(Transport::Udp)
            .expect("Could not get port"),
        handshake: lightning_interfaces::types::HandshakePorts {
            http: port_assigner
                .next_port(Transport::Tcp)
                .expect("Could not get port"),
            webrtc: port_assigner
                .next_port(Transport::Udp)
                .expect("Could not get port"),
            webtransport: port_assigner
                .next_port(Transport::Udp)
                .expect("Could not get port"),
        },
    }
}

/// Build the configuration object for a
fn build_config(
    root: &Path,
    ports: NodePorts,
    archiver: bool,
    syncronizer_delta: Duration,
) -> TomlConfigProvider<FinalTypes> {
    let config = TomlConfigProvider::<FinalTypes>::default();

    config.inject::<Resolver<FinalTypes>>(ResolverConfig {
        store_path: root
            .join("data/resolver_store")
            .try_into()
            .expect("Failed to resolve path"),
    });
    config.inject::<Rpc<FinalTypes>>(RpcConfig::default_with_port(ports.rpc));

    config.inject::<Consensus<FinalTypes>>(ConsensusConfig {
        store_path: root
            .join("data/narwhal_store")
            .try_into()
            .expect("Failed to resolve path"),
    });

    config.inject::<Keystore<FinalTypes>>(KeystoreConfig {
        node_key_path: root
            .join("keys/node.pem")
            .try_into()
            .expect("Failed to resolve path"),
        consensus_key_path: root
            .join("keys/consensus.pem")
            .try_into()
            .expect("Failed to resolve path"),
    });

    config.inject::<Blockstore<FinalTypes>>(BlockstoreConfig {
        root: root
            .join("data/blockstore")
            .try_into()
            .expect("Failed to resolve path"),
    });

    config.inject::<BlockstoreServer<FinalTypes>>(BlockstoreServerConfig::default());

    config.inject::<Handshake<FinalTypes>>(HandshakeConfig {
        // TODO: figure out how to have e2e testing for the different transports (browser oriented)
        transports: vec![TransportConfig::WebRTC(WebRtcConfig {
            address: ([0, 0, 0, 0], ports.handshake.webrtc).into(),
        })],
        http_address: ([127, 0, 0, 1], ports.handshake.http).into(),
        ..Default::default()
    });

    config.inject::<ServiceExecutor<FinalTypes>>(ServiceExecutorConfig {
        services: Default::default(),
        ..Default::default()
    });

    config.inject::<ReputationAggregator<FinalTypes>>(RepAggConfig {
        reporter_buffer_size: 1,
    });

    config.inject::<PoolProvider<FinalTypes>>(PoolConfig {
        address: format!("127.0.0.1:{}", ports.pool).parse().unwrap(),
        ..Default::default()
    });

    config.inject::<Syncronizer<FinalTypes>>(SyncronizerConfig {
        epoch_change_delta: syncronizer_delta,
    });

    config.inject::<Archive<FinalTypes>>(ArchiveConfig {
        is_archive: archiver,
        store_path: root
            .join("data/archive")
            .try_into()
            .expect("Failed to resolve path"),
    });

    config.inject::<Pinger<FinalTypes>>(PingerConfig {
        address: format!("127.0.0.1:{}", ports.pinger).parse().unwrap(),
        ping_interval: Duration::from_millis(1000),
    });
    config
}

/// Given the configuration of a node, generate and store the networking and consensus secret keys
/// of the node and write them into the path specified by the configuration of the signer.
///
/// Returns the public keys of the generated keys.
fn generate_and_store_node_secret(
    config: &TomlConfigProvider<FinalTypes>,
) -> (NodePublicKey, ConsensusPublicKey) {
    Keystore::<FinalTypes>::generate_keys(config.get::<Keystore<FinalTypes>>(), true)
        .expect("failed to ensure keys are generated");
    let keystore = Keystore::<FinalTypes>::init(config).expect("failed to load keystore");
    (keystore.get_ed25519_pk(), keystore.get_bls_pk())
}

/// Used to add more nodes to the swarm with specific settings.
#[derive(Clone)]
pub struct SwarmNode {
    pub reputation_score: Option<u8>,
    pub stake: Option<Staking>,
    pub is_committee: bool,
}
