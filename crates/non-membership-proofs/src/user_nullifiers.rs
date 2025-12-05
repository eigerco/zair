//! This module provides functionality for handling user nullifiers. Scans the remote chain,
//! identifies user nullifiers and returns them.

use futures_core::Stream;
use orchard::keys::FullViewingKey as OrchardFvk;
use sapling::zip32::DiversifiableFullViewingKey;
use zcash_primitives::consensus::Parameters;

use crate::user_nullifiers::decrypt_notes::{derive_orchard_nullifier, derive_sapling_nullifier};

pub(crate) mod decrypt_notes;

// Re-export viewing keys for external use
pub use decrypt_notes::{OrchardViewingKeys, SaplingViewingKeys, ViewingKeys};
pub use zip32::Scope;

/// Metadata common to all found notes (Sapling and Orchard)
#[derive(Debug, Clone)]
pub struct NoteMetadata {
    /// Block height where the note was found
    pub height: u64,
    /// Transaction ID containing the note
    pub txid: Vec<u8>,
    /// The scope (External for received payments, Internal for change)
    pub scope: Scope,
}

/// A note found for the user, with metadata
#[derive(Debug, Clone)]
pub enum FoundNote {
    /// Orchard note
    Orchard {
        /// The Orchard note
        note: orchard::Note,
        /// Common metadata
        metadata: NoteMetadata,
    },
    /// Sapling note
    Sapling {
        /// The Sapling note
        note: sapling::Note,
        /// Common metadata
        metadata: NoteMetadata,
        /// Position in Sapling commitment tree (required for nullifier derivation)
        position: u64,
    },
}

impl FoundNote {
    /// Get the common metadata for this note
    pub fn metadata(&self) -> &NoteMetadata {
        match self {
            FoundNote::Orchard { metadata, .. } => metadata,
            FoundNote::Sapling { metadata, .. } => metadata,
        }
    }

    /// Get note block height
    pub fn height(&self) -> u64 {
        self.metadata().height
    }

    /// Get note pool
    pub fn pool(&self) -> &'static str {
        match self {
            FoundNote::Orchard { .. } => "Orchard",
            FoundNote::Sapling { .. } => "Sapling",
        }
    }

    /// Returns the scope of this note (External for received, Internal for change)
    pub fn scope(&self) -> Scope {
        self.metadata().scope
    }

    /// Derive the nullifier for this note
    ///
    /// # Arguments
    /// * `viewing_keys` - The viewing keys containing FVK/NK needed for nullifier derivation
    ///
    /// # Returns
    /// The 32-byte nullifier that will be revealed when this note is spent
    pub fn nullifier(&self, viewing_keys: &ViewingKeys) -> [u8; 32] {
        match self {
            FoundNote::Sapling {
                note,
                metadata,
                position,
            } => {
                // Sapling nullifier derivation requires the note position and NK
                let sapling_keys = viewing_keys
                    .sapling
                    .as_ref()
                    .expect("Sapling viewing keys required for Sapling note");
                let nk = sapling_keys.nk(metadata.scope);
                derive_sapling_nullifier(note, nk, *position)
            }
            FoundNote::Orchard { note, .. } => {
                // Orchard nullifier derivation only requires the FVK
                let orchard_keys = viewing_keys
                    .orchard
                    .as_ref()
                    .expect("Orchard viewing keys required for Orchard note");
                derive_orchard_nullifier(note, &orchard_keys.fvk)
            }
        }
    }

    /// Get the airdrop nullifier for this note
    pub fn airdrop_nullifier(&self, viewing_keys: &ViewingKeys) -> [u8; 32] {
        match self {
            FoundNote::Sapling {
                note,
                metadata,
                position,
            } => {
                // Sapling nullifier derivation requires the note position and NK
                let sapling_keys = viewing_keys
                    .sapling
                    .as_ref()
                    .expect("Sapling viewing keys required for Sapling note");
                let nk = sapling_keys.nk(metadata.scope);
                note.nf_hiding(&nk, *position, b"TODO:personalization").0
            }
            FoundNote::Orchard { note, .. } => {
                // Orchard nullifier derivation only requires the FVK
                let orchard_keys = viewing_keys
                    .orchard
                    .as_ref()
                    .expect("Orchard viewing keys required for Orchard note");
                note.hiding_nullifier(&orchard_keys.fvk, "todo:domain", b"K")
                    .to_bytes()
            }
        }
    }
}

/// A trait for sources that can provide user nullifiers
pub trait UserNullifiers: Sized {
    /// The error type for this source
    type Error: std::error::Error + Send + 'static;

    /// The concrete stream type returned by this source
    type Stream: Stream<Item = Result<FoundNote, Self::Error>> + Send;

    /// Consume self and return a stream of all nullifiers (both Sapling and Orchard)
    ///
    /// TODO: handle cancellation
    fn user_nullifiers<P: Parameters + Clone + Send + 'static>(
        self,
        network: &P,
        start_height: u64,
        end_height: u64,
        orchard_fvk: &OrchardFvk,
        sapling_fvk: &DiversifiableFullViewingKey,
    ) -> Self::Stream;
}
