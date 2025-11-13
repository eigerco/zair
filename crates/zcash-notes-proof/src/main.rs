use std::io::Cursor;

use clap::Parser;
use eyre::{Result, WrapErr as _, eyre};
use orchard::keys::FullViewingKey as OrchardFvk;
use sapling_crypto::keys::FullViewingKey as SaplingFvk;
use tonic::transport::Endpoint;
use tracing::{debug, info};
use zcash_notes_proof::light_wallet_api::compact_tx_streamer_client::CompactTxStreamerClient;
use zcash_notes_proof::{FoundNote, find_user_notes};
use zcash_primitives::consensus::Network;

#[derive(Parser)]
#[command(name = "zcash-notes-proof")]
#[command(about = "Find Zcash notes using lightwalletd")]
struct Cli {
    /// Network type (mainnet or testnet)
    #[arg(long, env = "NETWORK", default_value = "testnet", value_parser = parse_network)]
    network: Network,

    /// Lightwalletd server URL (overrides default for network)
    #[arg(short = 'u', long, env = "LIGHTWALLETD_URL")]
    lightwalletd_url: Option<String>,

    /// Orchard Full Viewing Key (hex-encoded, 96 bytes)
    #[arg(short = 'o', long, env = "ORCHARD_FVK", value_parser = parse_orchard_fvk)]
    orchard_fvk: OrchardFvk,

    /// Sapling Full Viewing Key (hex-encoded, 96 bytes)
    #[arg(short = 's', long, env = "SAPLING_FVK", value_parser = parse_sapling_fvk)]
    sapling_fvk: SaplingFvk,

    /// Start block height
    #[arg(long, env = "START_HEIGHT")]
    start_height: u64,

    /// End block height
    #[arg(long, env = "END_HEIGHT")]
    end_height: u64,
}

impl Cli {
    fn network_config(&self) -> NetworkConfig {
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
struct NetworkConfig {
    network: Network,
    lightwalletd_url: String,
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

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file (fails silently if not found)
    dotenvy::dotenv().ok();

    // Initialize logging
    tracing_subscriber::fmt::init();

    // Initialize rustls crypto provider
    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|_| eyre!("Failed to install rustls crypto provider (already installed?)"))?;

    // Parse CLI arguments (includes env vars loaded from .env)
    let cli = Cli::parse();
    let config = cli.network_config();

    debug!(
        url = %config.lightwalletd_url,
        network = ?config.network,
        start = cli.start_height,
        end = cli.end_height,
        "Starting note search"
    );

    // Connect to lightwalletd
    let endpoint = Endpoint::from_shared(config.lightwalletd_url.clone())
        .wrap_err_with(|| format!("Invalid lightwalletd URL: {}", config.lightwalletd_url))?;
    let mut client = CompactTxStreamerClient::connect(endpoint)
        .await
        .wrap_err_with(|| {
            format!(
                "Failed to connect to lightwalletd at {}",
                config.lightwalletd_url
            )
        })?;

    // Find notes
    let notes = find_user_notes(
        &mut client,
        cli.start_height,
        cli.end_height,
        &cli.orchard_fvk,
        &cli.sapling_fvk,
        &config.network,
        Some(|h| info!(height = h, "Processing block")),
    )
    .await
    .wrap_err_with(|| {
        format!(
            "Failed to scan for notes in block range {} to {}",
            cli.start_height, cli.end_height
        )
    })?;

    // Display results
    display_results(&notes);

    Ok(())
}

fn display_results(notes: &[FoundNote]) {
    println!("\n{}", "=".repeat(50));
    println!("  Found {} note(s)", notes.len());
    println!("{}\n", "=".repeat(50));

    if notes.is_empty() {
        println!("No notes found in the specified range.");
        return;
    }

    for (i, found) in notes.iter().enumerate() {
        println!("┌─ Note #{}", i + 1);
        println!("│ Protocol:  {}", found.protocol());
        println!("│ Height:    {}", found.height());
        println!("│ Value:     {} zatoshis", found.value());

        if let FoundNote::Orchard { scope, .. } = found {
            println!("│ Scope:     {scope:?}");
        }

        let txid = match found {
            FoundNote::Orchard { txid, .. } | FoundNote::Sapling { txid, .. } => txid,
        };
        println!("│ TxID:      {}", hex::encode(txid));
        println!("└{}\n", "─".repeat(48));
    }
}
