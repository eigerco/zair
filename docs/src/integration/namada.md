# Integration: Namada

This page describes an example integration of ZAIR in Namada. You can find the latest source code hosted at [GitHub](https://github.com/eigerco/namada) (_latest commit at the time of writing: `f3292f8`_).

## Implementation

This section describes the Namada ZAIR implementation.

### Transactions

We introduce a new transaction type, `ClaimAirdrop`, with the following signature:

```rust
pub struct ClaimAirdrop {
    /// Token address to claim.
    pub token: Address,
    /// The target of the airdrop.
    pub target: Address,
    /// Claim data containing ZK proof information.
    pub claim_data: ClaimProofsOutput,
}
```

The transaction can be submitted either via the existing CLI or the SDK.

### Validity Predicate

Upon execution the transaction triggers a new custom validity predicate, `AirdropVP`, that runs a series of checks verifying the ZAIR airdrop. The steps to verify a ZAIR claim can be found outlined in .

### Storage

To support ZAIR integration, we extend Namdada's base storage with new keys for storing necessary ZAIR data: Sapling verification keys/Orchard parameters, note commitment roots, nullifier gap roots, target ids and airdrop nullifiers.

## Verification

This section details how ZAIR claim submissions are verified inside the validity predicate.

### Airdrop Nullifiers

To prevent double-spending airdrop nullifiers must be correctly tracked and deduplicated. For Namada, we introduce a new storage key and functions to manipulate the airdrop nullifier storage. Finally, we add additional checks inside the validity predicate asserting that airdrop nullifiers for a given action have not already been claimed, are unique, and flushed to the store correctly.

```rust
/// Checks if airdrop nullifiers have already been used.
fn check_airdrop_nullifiers<'ctx, CTX>(
    ctx: &'ctx CTX,
    claim_data: &ClaimProofsOutput,
    revealed_nullifiers: &mut HashSet<Key>,
) -> Result<()>
where
    CTX: VpEnv<'ctx> + namada_tx::action::Read<Err = Error>,
{
    for nullifier in claim_data.nullifier_iter() {
        let airdrop_nullifier_key = airdrop_nullifier_key(nullifier);

        // Check if nullifier has already been used before.
        if ctx.has_key_pre(&airdrop_nullifier_key)? {
            return Err(VpError::NullifierAlreadyUsed(reversed_hex_encode(
                nullifier,
            ))
            .into());
        }

        // Check if nullifier was previously used in this transaction.
        if revealed_nullifiers.contains(&airdrop_nullifier_key) {
            return Err(VpError::NullifierAlreadyUsed(reversed_hex_encode(
                nullifier,
            ))
            .into());
        }

        // Check that the nullifier was properly committed to store.
        ctx.read_bytes_post(&airdrop_nullifier_key)?
            .is_some_and(|value| value.is_empty())
            .then_some(())
            .ok_or(VpError::NullifierNotCommitted)?;

        revealed_nullifiers.insert(airdrop_nullifier_key);
    }

    Ok(())
}
```

See [Airdrop Nullifiers](../concepts/airdrop-nullifier.md) for more details.

### Message

ZAIR supports standard blob signatures for implementation-specific messages. For Namada we demonstrate an example of this by defining a custom `Message` used to verify claim submissions:

```rust
pub struct Message {
    /// The target of the airdrop.
    pub target: Address,
    /// Amount to claim.
    pub amount: u64,
    /// Commitment value randomness.
    pub rcv: [u8; 32],
}
```

A claimaint provides their binary-encoded message along with their proofs to ZAIR and signs the message to generate a standard signature cryptographically linking the message to the hash of the proof.

To verify the validity of the signature we first compute the message hash and the proof hash. Then, using ZAIR's public API we compute a signature digest and verify it:

```rust
/// Verifies that the Sapling spend-auth signature is valid.
fn verify_signature(
    target_id: &[u8],
    proof: &SaplingSignedClaim,
    message_hash: &[u8; 32],
) -> Result<()> {
    let proof_hash = hash_sapling_proof_fields(
        &proof.zkproof,
        &proof.rk,
        proof.cv,
        proof.cv_sha256,
        proof.airdrop_nullifier.into(),
    );

    let digest =
        signature_digest(Pool::Sapling, target_id, &proof_hash, message_hash)
            .map_err(|_| VpError::InvalidSpendAuthSignature)?;
    zair_sapling_proofs::verify_signature(
        proof.rk,
        proof.spend_auth_sig,
        &digest,
    )
    .map_err(|_| VpError::InvalidSpendAuthSignature)?;

    Ok(())
}
```

```admonish note
The signature uses both a target id and a searate pool identifier.
```

### Value Commitment

For the value commitment scheme we choose `SHA256`. Extracting `value` and `rcv` from the Namada message we compute the value commitment and assert that it's equal to the signed one:

```rust
/// Checks that the SHA256 value commitment is valid.
///
/// This computes that `cv = SHA256(b'Zair || LE64(amount) || rcv)`.
fn check_sha256_value_commitment(
    cv: &[u8; 32],
    Message { amount, rcv, .. }: &Message,
) -> Result<()> {
    let computed_cv = compute_cv_sha256(*amount, *rcv);
    if computed_cv != *cv {
        return Err(VpError::ValueCommitmentMismatch.into());
    }

    Ok(())
}
```

See [Value Commitments](../concepts/value-commitments.md) for more details.

### Proof Verification

Finally, the zero-knowledge proof is verified using ZAIR's public standard verifier API.
