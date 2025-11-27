use clap::Parser as _;
use eyre::{Result, WrapErr as _, eyre};
use light_wallet_api::ChainSpec;
use light_wallet_api::compact_tx_streamer_client::CompactTxStreamerClient;
use orchard::keys::FullViewingKey as OrchardFvk;
use sapling_crypto::keys::FullViewingKey as SaplingFvk;
use tonic::Request;
use tonic::transport::Endpoint;
use tracing::{debug, info};
use zcash_notes_proof::{
    FoundNote, collect_spent_nullifiers, derive_orchard_nullifier, derive_sapling_nullifier,
    find_user_notes,
};

mod cli;
use crate::cli::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file (fails silently if not found)
    if let Err(e) = dotenvy::dotenv() {
        eprintln!("Note: .env file not loaded: {}", e);
    } else {
        eprintln!("Loaded .env file");
    }

    // Debug: show current RUST_LOG setting
    if let Ok(rust_log) = std::env::var("RUST_LOG") {
        eprintln!("RUST_LOG is set to: {}", rust_log);
    } else {
        eprintln!("RUST_LOG is not set, using default: info");
    }

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Initialize rustls crypto provider
    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|_| eyre!("Failed to install rustls crypto provider (already installed?)"))?;

    // Parse CLI arguments (includes env vars loaded from .env)
    let cli = Cli::parse();
    let config = cli.network_config();

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

    // Determine end height: use provided value or query current chain tip
    let end_height = match cli.end_height {
        Some(height) => {
            info!("Using provided end height: {}", height);
            height
        }
        None => {
            info!("No end height provided, querying current chain tip...");
            let tip = get_latest_block_height(&mut client).await?;
            info!("Current chain tip: {}", tip);
            tip
        }
    };

    debug!(
        url = %config.lightwalletd_url,
        network = ?config.network,
        start = cli.start_height,
        end = end_height,
        "Starting note search"
    );

    // Find notes
    let notes = find_user_notes(
        &mut client,
        cli.start_height,
        end_height,
        &cli.orchard_fvk,
        &cli.sapling_fvk,
        &config.network,
        Some(|h| info!(height = h, "Scanning for notes at block")),
    )
    .await
    .wrap_err_with(|| {
        format!(
            "Failed to scan for notes in block range {} to {}",
            cli.start_height, end_height
        )
    })?;

    // Collect spent nullifiers from the blockchain
    info!("Collecting spent nullifiers to determine spend status...");
    let spent_nullifiers = collect_spent_nullifiers(
        &mut client,
        cli.start_height,
        end_height,
        Some(|h| info!(height = h, "Scanning for spent nullifiers at block")),
    )
    .await
    .wrap_err_with(|| {
        format!(
            "Failed to collect spent nullifiers in block range {} to {}",
            cli.start_height, end_height
        )
    })?;

    // Display results with spend status
    display_results(
        &notes,
        &cli.orchard_fvk,
        &cli.sapling_fvk,
        &spent_nullifiers,
    );

    Ok(())
}

/// Convert a TxID from protocol order to textual representation (byte-reversed hex)
/// As per protocol spec: "the byte-reversed and hex-encoded representation is
/// exclusively a textual representation of a txid"
fn txid_to_hex(txid: &[u8]) -> String {
    let mut reversed = txid.to_vec();
    reversed.reverse();
    hex::encode(reversed)
}

/// Get the current blockchain tip height from lightwalletd
async fn get_latest_block_height(
    client: &mut CompactTxStreamerClient<tonic::transport::Channel>,
) -> Result<u64> {
    let response = client
        .get_latest_block(Request::new(ChainSpec {}))
        .await
        .wrap_err("Failed to get latest block from lightwalletd")?;

    let block = response.into_inner();

    Ok(block.height)
}

fn display_results(
    notes: &[FoundNote],
    orchard_fvk: &OrchardFvk,
    sapling_fvk: &SaplingFvk,
    spent_nullifiers: &std::collections::HashSet<[u8; 32]>,
) {
    println!("\n{}", "=".repeat(50));
    println!("  Found {} note(s)", notes.len());
    println!("{}\n", "=".repeat(50));

    if notes.is_empty() {
        println!("No notes found in the specified range.");
        return;
    }

    let mut spent_count = 0usize;
    let mut unspent_count = 0usize;
    let mut spent_value = 0u64;
    let mut unspent_value = 0u64;

    for (i, found) in notes.iter().enumerate() {
        println!("┌─ Note #{}", i + 1);
        println!("│ Protocol:  {}", found.protocol());
        println!("│ Height:    {}", found.height());
        println!("│ Value:     {} zatoshis", found.value());
        if let Some(pos) = found.position() {
            println!("│ Position:  {}", pos);
        }

        let (nullifier, is_spent) = match found {
            FoundNote::Orchard { note, scope, .. } => {
                println!("│ Scope:     {scope:?}");

                // Derive and display the nullifier
                let nf = derive_orchard_nullifier(note, orchard_fvk);
                let spent = spent_nullifiers.contains(&nf);
                (nf, spent)
            }
            FoundNote::Sapling { note, position, .. } => {
                // Derive and display the nullifier
                let nf = derive_sapling_nullifier(note, sapling_fvk, *position);
                let spent = spent_nullifiers.contains(&nf);
                (nf, spent)
            }
        };

        println!("│ Nullifier: {}", hex::encode(nullifier));

        // Display spend status
        if is_spent {
            println!("│ Status:    SPENT ❌");
            spent_count += 1;
            spent_value += found.value();
        } else {
            println!("│ Status:    UNSPENT ✓");
            unspent_count += 1;
            unspent_value += found.value();
        }

        let txid = match found {
            FoundNote::Orchard { txid, .. } | FoundNote::Sapling { txid, .. } => txid,
        };
        println!("│ TxID:      {}", txid_to_hex(txid));
        println!("└{}\n", "─".repeat(48));
    }

    // Display summary
    println!("{}", "=".repeat(50));
    println!("  SUMMARY");
    println!("{}", "=".repeat(50));
    println!("Total notes found: {}", notes.len());
    println!(
        "Spent notes:       {} ({:.8} ZEC)",
        spent_count,
        spent_value as f64 / 100_000_000.0
    );
    println!(
        "Unspent notes:     {} ({:.8} ZEC)",
        unspent_count,
        unspent_value as f64 / 100_000_000.0
    );
    println!();
    println!("WARNING: 'sapling' pool results are note relaiable at the moment");
    println!("{}\n", "=".repeat(50));
}
