use anyhow::Result;
use orchard::Note;
use orchard::keys::{FullViewingKey, PreparedIncomingViewingKey, Scope};
use orchard::note::{ExtractedNoteCommitment, Nullifier, Rho};
use orchard::note_encryption::{CompactAction, OrchardDomain};
use tonic::Request;
use tonic::transport::Endpoint;
use zcash_note_encryption::{EphemeralKeyBytes, try_compact_note_decryption};
// You'll need these RPC types from the zcash-vote project
// Copy from src/cash.z.wallet.sdk.rpc.rs or include the crate
use zcash_vote::rpc::{
    BlockId, BlockRange, CompactOrchardAction, compact_tx_streamer_client::CompactTxStreamerClient,
};

/// A note found for the user, with metadata
#[derive(Debug, Clone)]
pub struct FoundNote {
    pub note: Note,
    pub height: u32,
    pub txid: Vec<u8>,
    pub position: usize,
    pub scope: Scope,
}

/// Find all Orchard notes belonging to a user in a block range
pub async fn find_user_notes(
    lwd_url: &str,
    start_height: u32,
    end_height: u32,
    fvk: &FullViewingKey,
    progress: Option<impl Fn(u32)>,
) -> Result<Vec<FoundNote>> {
    // Prepare viewing keys for both scopes (External and Internal)
    let ivk_external = fvk.to_ivk(Scope::External);
    let pivk_external = PreparedIncomingViewingKey::new(&ivk_external);

    let ivk_internal = fvk.to_ivk(Scope::Internal);
    let pivk_internal = PreparedIncomingViewingKey::new(&ivk_internal);

    // Connect to lightwalletd server
    let ep = Endpoint::from_shared(lwd_url.to_string())?;
    let mut client = CompactTxStreamerClient::connect(ep).await?;

    // Request block range
    let mut blocks = client
        .get_block_range(Request::new(BlockRange {
            start: Some(BlockId {
                height: start_height as u64,
                hash: vec![],
            }),
            end: Some(BlockId {
                height: end_height as u64,
                hash: vec![],
            }),
            spam_filter_threshold: 0,
        }))
        .await?
        .into_inner();

    let mut found_notes = Vec::new();
    let mut global_position = 0usize;

    // Iterate through each block
    while let Some(block) = blocks.message().await? {
        let height = block.height as u32;

        // Optional progress callback
        if let Some(ref progress_fn) = progress {
            if height % 1000 == 0 || height == end_height {
                progress_fn(height);
            }
        }

        // Process each transaction in the block
        for tx in block.vtx {
            let txid = tx.hash.clone();

            // Process each Orchard action in the transaction
            for action in tx.actions {
                // Try to decrypt with External scope
                if let Some(note) = try_decrypt_action(&pivk_external, &action)? {
                    found_notes.push(FoundNote {
                        note,
                        height,
                        txid: txid.clone(),
                        position: global_position,
                        scope: Scope::External,
                    });
                }

                // Try to decrypt with Internal scope
                if let Some(note) = try_decrypt_action(&pivk_internal, &action)? {
                    found_notes.push(FoundNote {
                        note,
                        height,
                        txid: txid.clone(),
                        position: global_position,
                        scope: Scope::Internal,
                    });
                }

                global_position += 1;
            }
        }
    }

    Ok(found_notes)
}

/// Try to decrypt an Orchard action with the given viewing key
fn try_decrypt_action(
    pivk: &PreparedIncomingViewingKey,
    action: &CompactOrchardAction,
) -> Result<Option<Note>> {
    // Extract action components
    let nf = Nullifier::from_bytes(&as_byte256(&action.nullifier)).unwrap();
    let rho = Rho::from_nf_old(nf);
    let domain = OrchardDomain::for_rho(rho);

    let cmx = ExtractedNoteCommitment::from_bytes(&as_byte256(&action.cmx)).unwrap();
    let ephemeral_key = EphemeralKeyBytes(as_byte256(&action.ephemeral_key));
    let ciphertext: [u8; 52] = action.ciphertext.clone().try_into().unwrap();

    let compact_action = CompactAction::from_parts(nf, cmx, ephemeral_key, ciphertext);

    // Attempt decryption
    let note = try_compact_note_decryption(&domain, pivk, &compact_action)
        .map(|(note, _addr, _memo)| note);

    Ok(note)
}

/// Helper to convert slice to 32-byte array
fn as_byte256(h: &[u8]) -> [u8; 32] {
    let mut hh = [0u8; 32];
    hh.copy_from_slice(h);
    hh
}

// Example usage:
#[cfg(test)]
mod example {
    use bip39::{Language, Mnemonic};
    use orchard::keys::SpendingKey;
    use zcash_primitives::constants::mainnet::COIN_TYPE;
    use zcash_primitives::zip32::AccountId;

    use super::*;

    async fn example_usage() -> Result<()> {
        // 1. Get user's Full Viewing Key from mnemonic
        let mnemonic = "your twelve word mnemonic phrase goes here like this example";
        let m = Mnemonic::parse_in_normalized(Language::English, mnemonic)?;
        let seed = m.to_seed("");
        let spk = SpendingKey::from_zip32_seed(&seed, COIN_TYPE, AccountId::ZERO).unwrap();
        let fvk = FullViewingKey::from(&spk);

        // 2. Find notes in a block range
        let notes = find_user_notes(
            "https://mainnet.lightwalletd.com:9067",
            2_000_000, // start height
            2_001_000, // end height
            &fvk,
            Some(|h| println!("Processing block {}", h)),
        )
        .await?;

        // 3. Display found notes
        println!("Found {} notes", notes.len());
        for (i, found) in notes.iter().enumerate() {
            println!(
                "Note {}: height={}, scope={:?}, value={}",
                i,
                found.height,
                found.scope,
                found.note.value().inner()
            );
        }

        Ok(())
    }
}
