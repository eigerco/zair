//! Command-line interface for airdrop cli application

use std::ops::RangeInclusive;

use clap::Parser;
use eyre::{Result, eyre};
use zcash_primitives::consensus::Network;

#[derive(Debug, Parser)]
#[command(name = "airdrop")]
#[command(about = "Zcash airdrop tool for building snapshots and finding notes")]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, clap::Subcommand)]
pub(crate) enum Commands {
    /// Build a snapshot of nullifiers from a source
    BuildAirdropConfiguration {
        #[command(flatten)]
        config: CommonArgs,
        /// Configuration output file
        #[arg(
            long,
            env = "CONFIGURATION_OUTPUT_FILE",
            default_value = "airdrop_configuration.json"
        )]
        configuration_output_file: String,
        #[arg(
            long,
            env = "SAPLING_SNAPSHOT_NULLIFIERS",
            default_value = "sapling-snapshot-nullifiers.bin"
        )]
        sapling_snapshot_nullifiers: String,
        #[arg(
            long,
            env = "ORCHARD_SNAPSHOT_NULLIFIERS",
            default_value = "orchard-snapshot-nullifiers.bin"
        )]
        orchard_snapshot_nullifiers: String,
    },
    /// Find notes in the nullifier set
    FindNotes {
        #[command(flatten)]
        config: CommonArgs,
    },
}

#[derive(Debug, clap::Args)]
pub(crate) struct CommonArgs {
    /// Network to use (mainnet or testnet)
    #[arg(long, env = "NETWORK", default_value = "testnet", value_parser = parse_network)]
    pub network: Network,

    /// Block range for the snapshot (e.g., 1000000..=1100000). Range is inclusive.
    #[arg(long, env = "SNAPSHOT", value_parser = parse_range)]
    pub snapshot: RangeInclusive<u64>,

    #[command(flatten)]
    pub source: SourceArgs,
}

#[derive(Debug, Clone, clap::Args)]
pub(crate) struct SourceArgs {
    /// Lightwalletd gRPC endpoint URL
    #[arg(long, env = "LIGHTWALLETD_URL")]
    pub lightwalletd_url: Option<String>,

    /// Input files in format: sapling_path,orchard_path
    #[arg(long, env = "INPUT_FILES")]
    pub input_files: Option<FileSourceArgs>,
}

#[derive(Debug, Clone)]
pub(crate) struct FileSourceArgs {
    pub sapling: String,
    pub orchard: String,
}

impl std::str::FromStr for FileSourceArgs {
    type Err = eyre::Report;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (sapling, orchard) = s
            .split_once(',')
            .ok_or_else(|| eyre!("Expected format: sapling_path,orchard_path"))?;
        Ok(Self {
            sapling: sapling.to_string(),
            orchard: orchard.to_string(),
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) enum Source {
    Lightwalletd { url: String },
    File { orchard: String, sapling: String },
}

impl TryFrom<SourceArgs> for Source {
    type Error = eyre::Report;

    fn try_from(args: SourceArgs) -> Result<Self, Self::Error> {
        match (args.lightwalletd_url, args.input_files) {
            (Some(url), None) => Ok(Source::Lightwalletd { url }),
            (None, Some(files)) => Ok(Source::File {
                orchard: files.orchard,
                sapling: files.sapling,
            }),
            (None, None) => Err(eyre!(
                "No source specified. Provide --lightwalletd-url OR --input-files sapling,orchard"
            )),
            (Some(_), Some(_)) => Err(eyre!(
                "Cannot specify both --lightwalletd-url and --input-files. Nullifiers mast come from a single source."
            )),
        }
    }
}

fn parse_range(s: &str) -> Result<RangeInclusive<u64>> {
    let (start, end) = s
        .split_once("..")
        .ok_or_else(|| eyre!("Invalid range format. Use START..END"))?;
    Ok(start.parse()?..=end.parse()?)
}

fn parse_network(s: &str) -> Result<Network> {
    match s {
        "mainnet" => Ok(Network::MainNetwork),
        "testnet" => Ok(Network::TestNetwork),
        other => Err(eyre!(
            "Invalid network: {other}. Expected 'mainnet' or 'testnet'."
        )),
    }
}
