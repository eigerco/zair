//! CLI command implementations for the airdrop crate.
//!
//! This module contains the core logic for each CLI subcommand.
//!
//! These commands interact with lightwalletd, process nullifiers for Sapling and Orchard pools,
//! and ensure data integrity for the airdrop process.

use std::path::PathBuf;

use crate::commands::airdrop_configuration::AirdropConfiguration;
use crate::unspent_notes_proofs::UnspentNotesProofs;

mod airdrop_claim;
mod airdrop_configuration;

pub use airdrop_claim::airdrop_claim;
pub use airdrop_configuration::{
    HidingFactor, OrchardHidingFactor, SaplingHidingFactor, build_airdrop_configuration,
};
use eyre::Context as _;

#[allow(clippy::print_stdout, reason = "Prints schema to stdout")]
pub fn airdrop_configuration_schema() -> eyre::Result<()> {
    let schema = schemars::schema_for!(AirdropConfiguration);
    let schema_str = serde_json::to_string_pretty(&schema)?;
    println!("Airdrop Configuration JSON Schema:\n{schema_str}");

    Ok(())
}

#[allow(
    clippy::print_stdout,
    reason = "Its WIP, for the moment is printing to stdout. The final version will save proofs to files."
)]
pub async fn construct_proofs(airdrop_claims_input_file: PathBuf) -> eyre::Result<()> {
    let raw_claims_data = tokio::fs::read_to_string(&airdrop_claims_input_file)
        .await
        .with_context(|| {
            format!(
                "Failed to read airdrop claims input file: {}",
                airdrop_claims_input_file.display()
            )
        })?;
    let _claims_data = serde_json::from_str::<UnspentNotesProofs>(&raw_claims_data)
        .context("Failed to parse UnspentNotesProofs")?;
    println!(
        "Constructing proofs for airdrop claims from file: {}\n{raw_claims_data}",
        airdrop_claims_input_file.display()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use non_membership_proofs::Pool;

    use super::*;
    use crate::unspent_notes_proofs::{
        NullifierProof, OrchardPrivateInputs, PrivateInputs, PublicInputs, SaplingPrivateInputs,
    };

    #[test]
    fn shcema_sanity_check() {
        let schema = schemars::schema_for!(AirdropConfiguration);
        let schema_str = serde_json::to_string_pretty(&schema);
        assert!(schema_str.is_ok());
    }

    #[tokio::test]
    async fn construct_proofs_sanity_check() {
        let temp_file = tempfile::NamedTempFile::new().expect("Failed to create temp file");
        let unspent_notes_proofs = UnspentNotesProofs::new(
            [0xAAu8; 32], // sapling_merkle_root
            [0xBBu8; 32], // orchard_merkle_root
            [
                (
                    Pool::Sapling,
                    vec![NullifierProof {
                        block_height: 1000,
                        public_inputs: PublicInputs {
                            hiding_nullifier: [2u8; 32],
                        },
                        private_inputs: PrivateInputs::Sapling(SaplingPrivateInputs {
                            nullifier: [3u8; 32],
                            note_commitment: [4u8; 32],
                            note_position: 1,
                            left_nullifier: [0u8; 32],
                            right_nullifier: [1u8; 32],
                            leaf_position: 0,
                            merkle_proof: vec![],
                        }),
                    }],
                ),
                (
                    Pool::Orchard,
                    vec![NullifierProof {
                        block_height: 1000,
                        public_inputs: PublicInputs {
                            hiding_nullifier: [2u8; 32],
                        },
                        private_inputs: PrivateInputs::Orchard(OrchardPrivateInputs {
                            nullifier: [3u8; 32],
                            note_commitment: [4u8; 32],
                            left_nullifier: [0u8; 32],
                            right_nullifier: [1u8; 32],
                            leaf_position: 0,
                            merkle_proof: vec![],
                        }),
                    }],
                ),
            ]
            .into_iter()
            .collect::<HashMap<Pool, Vec<NullifierProof>>>(),
        );
        serde_json::to_writer(&temp_file, &unspent_notes_proofs)
            .expect("Failed to write UnspentNotesProofs to temp file");

        let result = construct_proofs(temp_file.path().to_path_buf()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn construct_proofs_file_not_found() {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let file_path = dir.path().join("missing_file.txt");
        let result = construct_proofs(file_path).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn construct_proofs_parse_failed() {
        let temp_file = tempfile::NamedTempFile::new().expect("Failed to create temp file");
        let result = construct_proofs(temp_file.path().to_path_buf()).await;
        assert!(result.is_err());
    }

    #[test]
    fn airdrop_configuration_schema_sanity_check() {
        let result = airdrop_configuration_schema();
        assert!(result.is_ok());
    }
}
