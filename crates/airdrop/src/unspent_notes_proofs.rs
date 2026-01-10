//! Helpers for serializing nullifiers non-membership proofs.

use std::collections::HashMap;

use non_membership_proofs::utils::ReversedHex;
use non_membership_proofs::{Nullifier, Pool};
use serde::{Deserialize, Serialize};
use serde_with::hex::Hex;
use serde_with::serde_as;

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
pub struct UnspentNotesProofs {
    /// The merkle tree root that the proof is against for sapling pool.
    #[serde_as(as = "Hex")]
    pub sapling_merkle_root: [u8; 32],
    /// The merkle tree root that the proof is against for orchard pool.
    #[serde_as(as = "Hex")]
    pub orchard_merkle_root: [u8; 32],
    pools: HashMap<Pool, Vec<NullifierProof>>,
}

impl UnspentNotesProofs {
    /// Create a new `UnspentNotesProofs` from a map of pool proofs.
    #[must_use]
    pub const fn new(
        sapling_merkle_root: [u8; 32],
        orchard_merkle_root: [u8; 32],
        pools: HashMap<Pool, Vec<NullifierProof>>,
    ) -> Self {
        Self {
            sapling_merkle_root,
            orchard_merkle_root,
            pools,
        }
    }
}

/// A non-membership proof demonstrating that a nullifier is not in the snapshot.
///
/// This proof contains the two adjacent nullifiers that bound the target nullifier
/// (proving it falls in a "gap") along with a Merkle proof that this gap exists
/// in the committed snapshot.
#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
pub struct NullifierProof {
    /// The block height where the note was created.
    pub block_height: u64,
    /// The public inputs for the non-membership proof.
    pub public_inputs: PublicInputs,
    /// The private inputs for the non-membership proof.
    pub private_inputs: PrivateInputs,
}

/// Private inputs for the non-membership proof, specific to each pool.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "pool")]
pub enum PrivateInputs {
    /// Sapling pool private inputs
    Sapling(SaplingPrivateInputs),
    /// Orchard pool private inputs
    Orchard(OrchardPrivateInputs),
}

/// Private inputs for a Sapling non-membership proof.
#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
pub struct SaplingPrivateInputs {
    /// Nullifier being proven as not in the snapshot.
    #[serde_as(as = "ReversedHex")]
    pub nullifier: Nullifier,
    /// The commitment of the note that is unspent (reversed for Sapling display convention).
    #[serde_as(as = "ReversedHex")]
    pub note_commitment: [u8; 32],
    /// The position of the note in Sapling commitment tree.
    pub note_position: u64,
    /// The lower bound nullifier (the largest nullifier smaller than the target).
    #[serde_as(as = "ReversedHex")]
    pub left_nullifier: Nullifier,
    /// The upper bound nullifier (the smallest nullifier larger than the target).
    #[serde_as(as = "ReversedHex")]
    pub right_nullifier: Nullifier,
    /// The position of the leaf note in the Merkle tree.
    pub leaf_position: u64,
    /// The Merkle proof bytes proving the `(left, right)` range leaf exists in the tree.
    #[serde_as(as = "Hex")]
    pub merkle_proof: Vec<u8>,
}

/// Private inputs for an Orchard non-membership proof.
#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
pub struct OrchardPrivateInputs {
    /// Nullifier being proven as not in the snapshot.
    #[serde_as(as = "ReversedHex")]
    pub nullifier: Nullifier,
    /// The commitment of the note that is unspent.
    #[serde_as(as = "Hex")]
    pub note_commitment: [u8; 32],
    /// The lower bound nullifier (the largest nullifier smaller than the target).
    #[serde_as(as = "ReversedHex")]
    pub left_nullifier: Nullifier,
    /// The upper bound nullifier (the smallest nullifier larger than the target).
    #[serde_as(as = "ReversedHex")]
    pub right_nullifier: Nullifier,
    /// The position of the leaf note in the Merkle tree.
    pub leaf_position: u64,
    /// The Merkle proof bytes proving the `(left, right)` range leaf exists in the tree.
    #[serde_as(as = "Hex")]
    pub merkle_proof: Vec<u8>,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
pub struct PublicInputs {
    /// The hiding nullifier
    #[serde_as(as = "Hex")]
    pub hiding_nullifier: Nullifier,
}
