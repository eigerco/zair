/// Minimal version - Find user notes without database
///
/// This is a standalone module that scans Zcash blocks for user notes.
use eyre::{Result, WrapErr as _};
use orchard::keys::{
    FullViewingKey as OrchardFvk, PreparedIncomingViewingKey as OrchardPivk, Scope,
};
use orchard::note::{ExtractedNoteCommitment, Nullifier};
use orchard::note_encryption::{CompactAction, OrchardDomain};
use sapling_crypto::keys::FullViewingKey as SaplingFvk;
use sapling_crypto::note_encryption::{
    CompactOutputDescription, PreparedIncomingViewingKey as SaplingPivk, SaplingDomain,
};
use tonic::Request;
use tracing::{debug, error, info};
use zcash_note_encryption::{EphemeralKeyBytes, try_compact_note_decryption};
use zcash_primitives::consensus::Network;
use zcash_primitives::transaction::components::sapling::zip212_enforcement;

use crate::light_wallet_api::compact_tx_streamer_client::CompactTxStreamerClient;
use crate::light_wallet_api::{BlockId, BlockRange, CompactOrchardAction, CompactSaplingOutput};

/// A note found for the user, with metadata
#[derive(Debug, Clone)]
pub enum FoundNote {
    Orchard {
        note: orchard::Note,
        height: u64,
        txid: Vec<u8>,
        position: usize,
        scope: Scope,
    },
    Sapling {
        note: sapling_crypto::Note,
        height: u64,
        txid: Vec<u8>,
        position: usize,
    },
}

impl FoundNote {
    pub fn height(&self) -> u64 {
        match self {
            FoundNote::Orchard { height, .. } => *height,
            FoundNote::Sapling { height, .. } => *height,
        }
    }

    pub fn value(&self) -> u64 {
        match self {
            FoundNote::Orchard { note, .. } => note.value().inner(),
            FoundNote::Sapling { note, .. } => note.value().inner(),
        }
    }

    pub fn protocol(&self) -> &str {
        match self {
            FoundNote::Orchard { .. } => "Orchard",
            FoundNote::Sapling { .. } => "Sapling",
        }
    }
}

/// Find all Orchard and Sapling notes belonging to a user in a block range
pub async fn find_user_notes(
    client: &mut CompactTxStreamerClient<tonic::transport::Channel>,
    start_height: u64,
    end_height: u64,
    orchard_fvk: &OrchardFvk,
    sapling_fvk: &SaplingFvk,
    network_type: &Network,
    progress: Option<impl Fn(u64)>,
) -> Result<Vec<FoundNote>> {
    debug!("Preparing viewing keys...");

    // Prepare Orchard viewing keys for both scopes (External and Internal)
    let orchard_ivk_external = orchard_fvk.to_ivk(Scope::External);
    let orchard_pivk_external = OrchardPivk::new(&orchard_ivk_external);

    let orchard_ivk_internal = orchard_fvk.to_ivk(Scope::Internal);
    let orchard_pivk_internal = OrchardPivk::new(&orchard_ivk_internal);

    // Prepare Sapling viewing key
    let sapling_ivk = sapling_fvk.vk.ivk();
    let sapling_pivk = SaplingPivk::new(&sapling_ivk);

    debug!("Requesting blocks from {start_height} to {end_height}...",);

    // Request block range
    let mut blocks = client
        .get_block_range(Request::new(BlockRange {
            start: Some(BlockId {
                height: start_height,
                hash: vec![],
            }),
            end: Some(BlockId {
                height: end_height,
                hash: vec![],
            }),
            pool_types: vec![],
        }))
        .await
        .wrap_err_with(|| {
            format!(
                "Failed to fetch block range from lightwalletd (blocks {start_height} to {end_height})"
            )
        })?
        .into_inner();

    let mut found_notes = Vec::new();
    let mut global_position = 0usize;
    let mut blocks_processed = 0;
    let mut orchard_actions_processed = 0;

    debug!("Scanning blocks...");

    // Iterate through each block
    while let Some(block) = blocks
        .message()
        .await
        .wrap_err("Failed to receive next block from lightwalletd stream")?
    {
        let height = block.height;
        blocks_processed += 1;

        // Optional progress callback
        if let Some(ref progress_fn) = progress &&
            (height.is_multiple_of(1000) || height == end_height)
        {
            progress_fn(height);
        }

        // Process each transaction in the block
        for tx in block.vtx {
            let txid = tx.txid.clone();

            // Process each Orchard action in the transaction
            for action in tx.actions {
                orchard_actions_processed += 1;

                // Debug: print that we're processing an action
                if height.is_multiple_of(10000) && orchard_actions_processed % 100 == 0 {
                    debug!(
                        "  Processed {} Orchard actions so far at block {}",
                        orchard_actions_processed, height
                    );
                }

                // Helper to process decryption results
                let process_orchard = |pivk, scope: Scope| {
                    try_decrypt_orchard_output(pivk, &action)
                        .inspect_err(|e| error!("  Error decrypting with {scope:?} scope: {e}"))
                        .ok()
                        .flatten()
                        .map(|note| {
                            info!(
                                "  ✓ Found note ({scope:?}) at height {height} with value {}",
                                note.value().inner()
                            );
                            FoundNote::Orchard {
                                note,
                                height,
                                txid: txid.clone(),
                                position: global_position,
                                scope,
                            }
                        })
                };

                // Try both External and Internal scopes
                found_notes.extend(
                    [
                        process_orchard(&orchard_pivk_external, Scope::External),
                        process_orchard(&orchard_pivk_internal, Scope::Internal),
                    ]
                    .into_iter()
                    .flatten(),
                );

                global_position += 1;
            }

            // Process each Sapling output in the transaction
            for output in tx.outputs {
                // Try to decrypt Sapling output
                match try_decrypt_sapling_output(&sapling_pivk, &output, height, network_type) {
                    Ok(Some(note)) => {
                        info!(
                            "  ✓ Found Sapling note at height {height} with value {}",
                            note.value().inner()
                        );
                        found_notes.push(FoundNote::Sapling {
                            note,
                            height,
                            txid: txid.clone(),
                            position: global_position,
                        });
                    }
                    Ok(None) => {
                        // Note didn't decrypt - this is normal
                    }
                    Err(e) => {
                        error!("  Error decrypting Sapling output: {e}");
                    }
                }

                global_position += 1;
            }
        }
    }

    debug!("Scanning complete!");
    debug!("Blocks processed: {blocks_processed}",);
    debug!("Orchard actions processed: {orchard_actions_processed}");
    debug!("Total notes found: {}", found_notes.len());

    Ok(found_notes)
}

/// Try to decrypt an Orchard action with the given viewing key
fn try_decrypt_orchard_output(
    pivk: &OrchardPivk,
    action: &CompactOrchardAction,
) -> Result<Option<orchard::Note>> {
    // Extract action components - return None if any component is invalid
    let nf_option = Nullifier::from_bytes(&as_byte256(&action.nullifier));
    if nf_option.is_none().into() {
        // Invalid nullifier, skip this action
        return Ok(None);
    }
    let nf = nf_option.unwrap();

    let cmx_option = ExtractedNoteCommitment::from_bytes(&as_byte256(&action.cmx));
    if cmx_option.is_none().into() {
        // Invalid commitment, skip this action
        return Ok(None);
    }
    let cmx = cmx_option.unwrap();

    let ephemeral_key = EphemeralKeyBytes(as_byte256(&action.ephemeral_key));

    let ciphertext: [u8; 52] = match action.ciphertext.clone().try_into() {
        Ok(c) => c,
        Err(_) => {
            // Wrong ciphertext length, skip this action
            return Ok(None);
        }
    };

    // Create compact action - the domain is derived from it
    let compact_action = CompactAction::from_parts(nf, cmx, ephemeral_key, ciphertext);
    let domain = OrchardDomain::for_compact_action(&compact_action);

    // Attempt decryption
    let note =
        try_compact_note_decryption(&domain, pivk, &compact_action).map(|(note, _addr)| note);

    Ok(note)
}

/// Try to decrypt a Sapling output with the given viewing key
fn try_decrypt_sapling_output(
    pivk: &SaplingPivk,
    output: &CompactSaplingOutput,
    height: u64,
    network_type: &Network,
) -> Result<Option<sapling_crypto::Note>> {
    // Extract output components
    let cmu_bytes = match output.cmu.as_slice().try_into() {
        Ok(bytes) => bytes,
        Err(_) => return Ok(None),
    };
    let cmu = sapling_crypto::note::ExtractedNoteCommitment::from_bytes(&cmu_bytes);
    if cmu.is_none().into() {
        return Ok(None);
    }

    let ephemeral_key = EphemeralKeyBytes(match output.ephemeral_key.as_slice().try_into() {
        Ok(bytes) => bytes,
        Err(_) => return Ok(None),
    });

    let enc_ciphertext: [u8; 52] = match output.ciphertext.clone().try_into() {
        Ok(c) => c,
        Err(_) => return Ok(None),
    };

    // Create compact output
    let compact_output = CompactOutputDescription {
        cmu: cmu.unwrap(),
        ephemeral_key,
        enc_ciphertext,
    };

    // Determine ZIP 212 enforcement based on height
    let zip212_enforcement = zip212_enforcement(
        network_type,
        zcash_primitives::consensus::BlockHeight::from_u32(
            height
                .try_into()
                .wrap_err_with(|| format!("Block height {height} exceeds u32::MAX"))?,
        ),
    );

    let domain = SaplingDomain::new(zip212_enforcement);

    // Attempt decryption
    let note =
        try_compact_note_decryption(&domain, pivk, &compact_output).map(|(note, _addr)| note);

    Ok(note)
}

/// Helper to convert slice to 32-byte array
fn as_byte256(h: &[u8]) -> [u8; 32] {
    let mut hh = [0u8; 32];
    hh.copy_from_slice(h);
    hh
}
