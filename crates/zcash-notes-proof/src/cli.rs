use std::io::Cursor;

use clap::Parser;
use eyre::{Result, WrapErr as _, eyre};
use orchard::keys::FullViewingKey as OrchardFvk;
use sapling::keys::FullViewingKey as SaplingFvk;
use zcash_primitives::consensus::Network;

#[derive(Parser)]
#[command(name = "zcash-notes-proof")]
#[command(about = "Find Zcash notes using lightwalletd")]
pub struct Cli {
    /// Network type (mainnet or testnet)
    #[arg(long, env = "NETWORK", default_value = "testnet", value_parser = parse_network)]
    pub network: Network,

    /// Lightwalletd server URL (overrides default for network)
    #[arg(short = 'u', long, env = "LIGHTWALLETD_URL")]
    pub lightwalletd_url: Option<String>,

    /// Orchard Full Viewing Key (hex-encoded, 96 bytes)
    #[arg(short = 'o', long, env = "ORCHARD_FVK", value_parser = parse_orchard_fvk)]
    pub orchard_fvk: OrchardFvk,

    /// Sapling Full Viewing Key (hex-encoded, 96 bytes)
    #[arg(short = 's', long, env = "SAPLING_FVK", value_parser = parse_sapling_fvk)]
    pub sapling_fvk: SaplingFvk,

    /// Start block height
    #[arg(long, env = "START_HEIGHT")]
    pub start_height: u64,

    /// End block height (optional - defaults to current chain tip)
    #[arg(long, env = "END_HEIGHT")]
    pub end_height: Option<u64>,
}

impl Cli {
    pub fn network_config(&self) -> NetworkConfig {
        let lightwalletd_url = self
            .lightwalletd_url
            .clone()
            .unwrap_or_else(|| NetworkConfig::default_url(&self.network).to_string());

        NetworkConfig {
            network: self.network,
            lightwalletd_url,
        }
    }
}

/// Network configuration with default lightwalletd endpoints
#[derive(Clone)]
pub struct NetworkConfig {
    pub network: Network,
    pub lightwalletd_url: String,
}

impl NetworkConfig {
    fn default_url(network: &Network) -> &'static str {
        match network {
            Network::MainNetwork => "https://zec.rocks:443",
            Network::TestNetwork => "https://testnet.zec.rocks:443",
        }
    }
}

fn parse_network(s: &str) -> Result<Network> {
    match s {
        "mainnet" => Ok(Network::MainNetwork),
        "testnet" => Ok(Network::TestNetwork),
        other => Err(eyre!(
            "Invalid network type: {other}. Expected 'mainnet' or 'testnet'.",
        )),
    }
}

/// Parse hex-encoded Orchard Full Viewing Key
fn parse_orchard_fvk(hex: &str) -> Result<OrchardFvk> {
    let bytes = hex::decode(hex).wrap_err("Failed to decode Orchard FVK from hex string")?;

    let bytes: [u8; 96] = bytes.try_into().map_err(|v: Vec<u8>| {
        eyre!(
            "Invalid Orchard FVK length: expected 96 bytes, got {} bytes",
            v.len()
        )
    })?;

    OrchardFvk::from_bytes(&bytes)
        .ok_or_else(|| eyre!("Invalid Orchard FVK: failed to parse 96-byte representation"))
}

/// Parse hex-encoded Sapling Full Viewing Key
fn parse_sapling_fvk(hex: &str) -> Result<SaplingFvk> {
    let bytes = hex::decode(hex).wrap_err("Failed to decode Sapling FVK from hex string")?;

    if bytes.len() != 96 {
        return Err(eyre!(
            "Invalid Sapling FVK length: expected 96 bytes, got {} bytes",
            bytes.len()
        ));
    }

    SaplingFvk::read(&mut Cursor::new(bytes))
        .wrap_err("Invalid Sapling FVK: failed to parse 96-byte representation")
}
