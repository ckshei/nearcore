use std::convert::TryInto;
use std::fs;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use log::info;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use serde_derive::{Deserialize, Serialize};

use near_client::BlockProducer;
use near_client::ClientConfig;
use near_jsonrpc::RpcConfig;
use near_network::NetworkConfig;
use near_primitives::crypto::signer::{InMemorySigner, KeyFile};
use near_primitives::types::{AccountId, Balance, ReadablePublicKey};

/// Initial balance used in tests.
pub const TESTING_INIT_BALANCE: Balance = 1_000_000_000_000;
/// Stake used by authorities to validate used in tests.
pub const TESTING_INIT_STAKE: Balance = 50_000_000;

pub const CONFIG_FILENAME: &str = "config.json";
pub const GENESIS_CONFIG_FILENAME: &str = "genesis.json";
pub const NODE_KEY_FILE: &str = "node_key.json";
pub const VALIDATOR_KEY_FILE: &str = "validator_key.json";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Network {
    /// Address to listen for incoming connections.
    pub addr: String,
    /// Address to advertise to peers for them to connect.
    /// If empty, will use the same port as the addr, and will introspect on the listener.
    pub external_address: String,
    /// Comma separated list of nodes to connect to.
    pub boot_nodes: String,
    /// Maximum number of peers.
    pub max_peers: u32,
    /// Handshake timeout.
    pub handshake_timeout: Duration,
    /// Duration before trying to reconnect to a peer.
    pub reconnect_delay: Duration,
    /// Skip waiting for peers before starting node.
    pub skip_sync_wait: bool,
}

impl Default for Network {
    fn default() -> Self {
        Network {
            addr: "0.0.0.0:24567".to_string(),
            external_address: "".to_string(),
            boot_nodes: "".to_string(),
            max_peers: 40,
            handshake_timeout: Duration::from_secs(20),
            reconnect_delay: Duration::from_secs(60),
            skip_sync_wait: false,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Consensus {
    /// Minimum number of peers to start syncing.
    pub min_num_peers: usize,
    /// Minimum duration before producing block.
    pub min_block_production_delay: Duration,
    /// Maximum duration before producing block or skipping height.
    pub max_block_production_delay: Duration,
    /// Produce empty blocks, use `false` for testing.
    pub produce_empty_blocks: bool,
}

impl Default for Consensus {
    fn default() -> Self {
        Consensus {
            min_num_peers: 3,
            min_block_production_delay: Duration::from_secs(1),
            max_block_production_delay: Duration::from_secs(6),
            produce_empty_blocks: true,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Config {
    pub genesis_file: String,
    pub validator_key_file: String,
    pub node_key_file: String,
    pub rpc: RpcConfig,
    pub network: Network,
    pub consensus: Consensus,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            genesis_file: GENESIS_CONFIG_FILENAME.to_string(),
            validator_key_file: VALIDATOR_KEY_FILE.to_string(),
            node_key_file: NODE_KEY_FILE.to_string(),
            rpc: RpcConfig::default(),
            network: Network::default(),
            consensus: Consensus::default(),
        }
    }
}

impl Config {
    pub fn from_file(path: &PathBuf) -> Self {
        let mut file = File::open(path).expect("Could not open config file.");
        let mut content = String::new();
        file.read_to_string(&mut content).expect("Could not read from config file.");
        Config::from(content.as_str())
    }

    pub fn write_to_file(&self, path: &PathBuf) {
        let mut file = File::create(path).expect("Failed to create / write a config file.");
        let str = serde_json::to_string_pretty(self).expect("Error serializing the config.");
        if let Err(err) = file.write_all(str.as_bytes()) {
            panic!("Failed to write a config file {}", err);
        }
    }
}

impl From<&str> for Config {
    fn from(content: &str) -> Self {
        serde_json::from_str(content).expect("Failed to deserialize config")
    }
}

#[derive(Clone)]
pub struct NearConfig {
    pub client_config: ClientConfig,
    pub network_config: NetworkConfig,
    pub rpc_config: RpcConfig,
    pub block_producer: Option<BlockProducer>,
    pub genesis_config: GenesisConfig,
}

impl NearConfig {
    pub fn new(
        config: Config,
        genesis_config: &GenesisConfig,
        network_key_pair: KeyFile,
        block_producer: Option<&BlockProducer>,
    ) -> Self {
        NearConfig {
            client_config: ClientConfig {
                chain_id: genesis_config.chain_id.clone(),
                rpc_addr: config.rpc.addr.clone(),
                min_block_production_delay: Duration::from_millis(100),
                max_block_production_delay: Duration::from_millis(2000),
                block_expected_weight: 1000,
                skip_sync_wait: config.network.skip_sync_wait,
                sync_check_period: Duration::from_secs(10),
                sync_step_period: Duration::from_millis(10),
                sync_weight_threshold: 0,
                sync_height_threshold: 1,
                min_num_peers: 1,
                fetch_info_period: Duration::from_millis(100),
                log_summary_period: Duration::from_secs(10),
                produce_empty_blocks: config.consensus.produce_empty_blocks,
            },
            network_config: NetworkConfig {
                public_key: network_key_pair.public_key,
                secret_key: network_key_pair.secret_key,
                account_id: block_producer.map(|bp| bp.account_id.clone()),
                addr: if config.network.addr.is_empty() {
                    None
                } else {
                    Some(config.network.addr.parse().unwrap())
                },
                boot_nodes: if config.network.boot_nodes.is_empty() {
                    vec![]
                } else {
                    config
                        .network
                        .boot_nodes
                        .split(",")
                        .map(|chunk| chunk.try_into().unwrap())
                        .collect()
                },
                handshake_timeout: config.network.handshake_timeout,
                reconnect_delay: config.network.reconnect_delay,
                bootstrap_peers_period: Duration::from_secs(60),
                peer_max_count: config.network.max_peers,
                // TODO: push this into config.
                ban_window: Duration::from_secs(3 * 60 * 60),
                max_send_peers: 512,
                peer_expiration_duration: Duration::from_secs(7 * 24 * 60 * 60),
            },
            rpc_config: config.rpc,
            genesis_config: genesis_config.clone(),
            block_producer: block_producer.map(Clone::clone),
        }
    }
}

/// Runtime configuration, defining genesis block.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GenesisConfig {
    /// Official time of blockchain start.
    pub genesis_time: DateTime<Utc>,
    /// ID of the blockchain. This must be unique for every blockchain.
    /// If your testnet blockchains do not have unique chain IDs, you will have a bad time.
    pub chain_id: String,
    /// Number of shards at genesis.
    pub num_shards: u32,
    /// List of initial authorities / validators.
    pub authorities: Vec<(AccountId, ReadablePublicKey, Balance)>,
    /// List of accounts / balances at genesis.
    pub accounts: Vec<(AccountId, ReadablePublicKey, Balance)>,
    /// List of contract code per accounts. Contract code encoded in base64.
    pub contracts: Vec<(AccountId, String)>,
}

impl GenesisConfig {
    pub fn legacy_test(seeds: Vec<&str>, num_validators: usize) -> Self {
        let mut authorities = vec![];
        let mut accounts = vec![];
        let mut contracts = vec![];
        let default_test_contract = base64::encode(
            include_bytes!("../../runtime/wasm/runtest/res/wasm_with_mem.wasm").as_ref(),
        );
        for (i, account) in seeds.iter().enumerate() {
            let signer = InMemorySigner::from_seed(account, account);
            if i < num_validators {
                authorities.push((
                    account.to_string(),
                    signer.public_key.to_readable(),
                    TESTING_INIT_STAKE,
                ));
            }
            accounts.push((
                account.to_string(),
                signer.public_key.to_readable(),
                TESTING_INIT_BALANCE,
            ));
            contracts.push((account.to_string(), default_test_contract.clone()))
        }
        GenesisConfig {
            genesis_time: Utc::now(),
            chain_id: random_chain_id(),
            num_shards: 1,
            authorities,
            accounts,
            contracts,
        }
    }

    pub fn test(seeds: Vec<&str>) -> Self {
        let num_validators = seeds.len();
        Self::legacy_test(seeds, num_validators)
    }

    pub fn testing_spec(num_accounts: usize, num_authorities: usize) -> Self {
        let mut accounts = vec![];
        let mut authorities = vec![];
        for i in 0..num_accounts {
            let account_id = format!("near.{}", i);
            let signer = InMemorySigner::from_seed(&account_id, &account_id);
            if i < num_authorities {
                authorities.push((
                    account_id.clone(),
                    signer.public_key.to_readable(),
                    TESTING_INIT_STAKE,
                ));
            }
            accounts.push((account_id, signer.public_key.to_readable(), TESTING_INIT_BALANCE));
        }
        GenesisConfig {
            genesis_time: Utc::now(),
            chain_id: random_chain_id(),
            num_shards: 1,
            authorities,
            accounts,
            contracts: vec![],
        }
    }

    /// Reads GenesisConfig from a file.
    pub fn from_file(path: &PathBuf) -> Self {
        let mut file = File::open(path).expect("Could not open genesis config file.");
        let mut content = String::new();
        file.read_to_string(&mut content).expect("Could not read from genesis config file.");
        GenesisConfig::from(content.as_str())
    }

    /// Writes GenesisConfig to the file.
    pub fn write_to_file(&self, path: &PathBuf) {
        let mut file = File::create(path).expect("Failed to create / write a genesis config file.");
        let str =
            serde_json::to_string_pretty(self).expect("Error serializing the genesis config.");
        if let Err(err) = file.write_all(str.as_bytes()) {
            panic!("Failed to write a genesis config file {}", err);
        }
    }
}

impl From<&str> for GenesisConfig {
    fn from(config: &str) -> Self {
        serde_json::from_str(config).expect("Failed to deserialize the genesis config.")
    }
}

fn random_chain_id() -> String {
    format!("test-chain-{}", thread_rng().sample_iter(&Alphanumeric).take(5).collect::<String>())
}

/// Initializes genesis and client configs and stores in the given folder
pub fn init_configs(dir: &Path, chain_id: Option<&str>, account_id: Option<&str>) {
    fs::create_dir_all(dir).expect("Failed to create directory");
    let chain_id = chain_id.map(|c| c.to_string()).unwrap_or(random_chain_id());
    match chain_id.as_ref() {
        "testnet" => {
            // TODO:
            unimplemented!();
        }
        "mainnet" => {
            // TODO:
            unimplemented!();
        }
        _ => {
            // Create new configuration, key files and genesis for one validator.
            let mut config = Config::default();
            config.network.skip_sync_wait = true;
            config.write_to_file(&dir.join(CONFIG_FILENAME));

            let account_id = account_id.unwrap_or("test.near").to_string();

            let signer = InMemorySigner::new(account_id.clone());
            signer.write_to_file(&dir.join(config.validator_key_file));

            let network_signer = InMemorySigner::new("".to_string());
            network_signer.write_to_file(&dir.join(config.node_key_file));

            let genesis_config = GenesisConfig {
                genesis_time: Utc::now(),
                chain_id,
                num_shards: 1,
                authorities: vec![(
                    account_id.clone(),
                    signer.public_key.to_readable(),
                    TESTING_INIT_STAKE,
                )],
                accounts: vec![(account_id, signer.public_key.to_readable(), TESTING_INIT_BALANCE)],
                contracts: vec![],
            };
            genesis_config.write_to_file(&dir.join(config.genesis_file));
            info!(target: "near", "Generated node key, validator key, genesis file in {}", dir.to_str().unwrap());
        }
    }
}

/// Create testnet configuration. If `local_ports` is true,
/// sets up new ports for all nodes except the first one and sets boot node to it.
pub fn create_testnet_configs(
    num_validators: usize,
    num_non_validators: usize,
    prefix: &str,
    local_ports: bool,
) -> (Vec<Config>, Vec<InMemorySigner>, Vec<InMemorySigner>, GenesisConfig) {
    let signers = (0..(num_validators + num_non_validators))
        .map(|i| InMemorySigner::new(format!("{}{}", prefix, i)))
        .collect::<Vec<_>>();
    let network_signers = (0..(num_validators + num_non_validators))
        .map(|_| InMemorySigner::from_random())
        .collect::<Vec<_>>();
    let accounts = (0..(num_validators + num_non_validators))
        .map(|i| {
            (format!("{}{}", prefix, i), signers[i].public_key.to_readable(), TESTING_INIT_BALANCE)
        })
        .collect::<Vec<_>>();
    let authorities = (0..(num_validators))
        .map(|i| {
            (format!("{}{}", prefix, i), signers[i].public_key.to_readable(), TESTING_INIT_STAKE)
        })
        .collect::<Vec<_>>();
    let genesis_config = GenesisConfig {
        genesis_time: Utc::now(),
        chain_id: random_chain_id(),
        num_shards: 1,
        authorities,
        accounts,
        contracts: vec![],
    };
    let mut configs = vec![];
    for i in 0..(num_validators + num_non_validators) {
        let mut config = Config::default();
        if local_ports {
            config.network.addr = format!("0.0.0.0:{}", if i == 0 { 26567 } else { 0 });
            config.rpc.addr = format!("0.0.0.0:{}", 3030 + i);
            config.network.boot_nodes = if i == 0 {
                "".to_string()
            } else {
                format!("{}@127.0.0.1:26567", network_signers[0].public_key)
            };
            config.network.skip_sync_wait = num_validators == 1;
        }
        configs.push(config);
    }
    (configs, signers, network_signers, genesis_config)
}

pub fn init_testnet_configs(
    dir: &Path,
    num_validators: usize,
    num_non_validators: usize,
    prefix: &str,
) {
    let (configs, signers, network_signers, genesis_config) =
        create_testnet_configs(num_validators, num_non_validators, prefix, false);
    for i in 0..(num_validators + num_non_validators) {
        let node_dir = dir.join(format!("{}{}", prefix, i));
        fs::create_dir_all(node_dir.clone()).expect("Failed to create directory");

        signers[i].write_to_file(&node_dir.join(configs[i].validator_key_file.clone()));
        network_signers[i].write_to_file(&node_dir.join(configs[i].node_key_file.clone()));

        genesis_config.write_to_file(&node_dir.join(configs[i].genesis_file.clone()));
        configs[i].write_to_file(&node_dir.join(CONFIG_FILENAME));
        info!(target: "near", "Generated node key, validator key, genesis file in {}", node_dir.to_str().unwrap());
    }
}

pub fn load_config(dir: &Path) -> NearConfig {
    let config = Config::from_file(&dir.join(CONFIG_FILENAME));
    let genesis_config = GenesisConfig::from_file(&dir.join(config.genesis_file.clone()));
    let signer = Arc::new(InMemorySigner::from_file(&dir.join(config.validator_key_file.clone())));
    let block_producer = BlockProducer::from(signer);
    let network_signer = InMemorySigner::from_file(&dir.join(config.node_key_file.clone()));
    NearConfig::new(
        config,
        &genesis_config,
        network_signer.into(),
        Some(&block_producer),
    )
}

pub fn load_test_config(seed: &str, port: u64, genesis_config: &GenesisConfig) -> NearConfig {
    let mut config = Config::default();
    config.network.addr = format!("0.0.0.0:{}", port);
    config.rpc.addr = format!("0.0.0.0:{}", port + 100);
    let signer = Arc::new(InMemorySigner::from_seed(seed, seed));
    let block_producer = BlockProducer::from(signer.clone());
    NearConfig::new(
        config,
        &genesis_config,
        signer.into(),
        Some(&block_producer),
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use near_primitives::types::ReadablePublicKey;

    use super::GenesisConfig;

    #[test]
    fn test_deserialize() {
        let data = json!({
            "genesis_time": "2019-05-07T00:10:14.434719Z",
            "chain_id": "test-chain-XYQAS",
            "num_shards": 1,
            "accounts": [["alice.near", "6fgp5mkRgsTWfd5UWw1VwHbNLLDYeLxrxw3jrkCeXNWq", 100]],
            "authorities": [("alice.near", "6fgp5mkRgsTWfd5UWw1VwHbNLLDYeLxrxw3jrkCeXNWq", 50)],
            "contracts": [],
        });
        let spec = GenesisConfig::from(data.to_string().as_str());
        assert_eq!(
            spec.authorities[0],
            (
                "alice.near".to_string(),
                ReadablePublicKey("6fgp5mkRgsTWfd5UWw1VwHbNLLDYeLxrxw3jrkCeXNWq".to_string()),
                50
            )
        );
    }
}
