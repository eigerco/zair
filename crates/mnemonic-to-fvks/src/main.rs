//! A utility to convert a Zcash mnemonic to Full Viewing Keys
//! Supports deriving Full Viewing Keys for Sapling or Orchard.
use std::path::PathBuf;

use clap::Parser;
use eyre::{Result, WrapErr as _};
use mnemonic_to_fvks::{CoinType, Pool, mnemonic_to_fvks, read_mnemonic_secure};
use zeroize::Zeroize as _;

#[derive(Parser)]
#[command(name = "mnemonic-to-fvks")]
#[command(about = "A utility to convert a Zcash mnemonic to Full Viewing Keys", long_about = None)]
struct Cli {
    /// Select the pool(s) to derive FVKs for. Default is Both. Available options: [sapling,
    /// orchard, both]
    #[arg(short, long, value_enum, default_value_t = Pool::Both)]
    pool: Pool,

    /// Specify the coin type for key derivation. Default is Testnet. Available options: [mainnet,
    /// testnet, regtest]
    #[arg(short = 'c', long, value_enum, default_value_t = CoinType::Testnet)]
    coin_type: CoinType,
}

#[allow(clippy::print_stdout, reason = "CLI utility")]
fn main() -> Result<()> {
    let _: Option<PathBuf> = dotenvy::dotenv().ok();
    let cli = Cli::parse();

    let mut mnemonic = read_mnemonic_secure()
        .wrap_err("Failed to read mnemonic from environment or user input")?;

    println!("Deriving Full Viewing Keys from mnemonic...\n");
    let (orchard_fvk, sapling_fvk) =
        mnemonic_to_fvks(&mnemonic, cli.coin_type).wrap_err_with(|| {
            format!(
                "Failed to derive Full Viewing Keys for coin type {:?}",
                cli.coin_type
            )
        })?;
    mnemonic.zeroize();

    println!("=== Full Viewing Keys (hex-encoded) ===\n");
    println!("Orchard FVK: '{}'", hex::encode(orchard_fvk.to_bytes()));
    println!("Sapling FVK: '{}'", hex::encode(sapling_fvk.to_bytes()));

    Ok(())
}
