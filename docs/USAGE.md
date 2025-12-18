# Usage Guide

> **Work in Progress**: This project is under active development. Not all features described in this guide are fully implemented yet, and some details may change.

This guide walks you through the complete workflow for:

1. **Airdrop organizers**: Building an airdrop snapshot configuration from Zcash blockchain data
2. **Users**: Claiming an airdrop using privacy-preserving non-membership proofs

## Overview

**As a Zcash user** with shielded funds (Sapling or Orchard notes) who wants to participate in the Namada airdrop, **you need to prove** that you held unspent funds at a specific blockchain height **without revealing your actual nullifiers** (which would compromise your privacy).

### The Problem

Zcash's shielded pools use nullifiers to track spent notes. When you spend a note, its nullifier gets published to the blockchain. To prove you had unspent funds at the airdrop snapshot, you'd normally need to show your nullifier isn't in the set of spent nullifiers—but revealing your nullifier would link your note to your identity.

### The Solution

This toolkit generates **non-membership proofs**—cryptographic proofs that demonstrate your note's nullifier is NOT in the snapshot of spent nullifiers, without revealing the nullifier itself. These proofs use "hiding nullifiers" (a privacy-preserving derivation) and Merkle tree proofs.

## Workflow

### Step 1: Build the Airdrop Snapshot

Run `build-airdrop-configuration` to:

1. Fetch all nullifiers from the blockchain up to the snapshot height
2. Build Merkle trees for Sapling and Orchard pools
3. Export snapshot files and a configuration JSON with Merkle roots

```bash
airdrop build-airdrop-configuration \
  --snapshot 419200..=2500000 \
  --lightwalletd-url https://lightwalletd.example.com \
  --configuration-output-file airdrop_configuration.json \
  --sapling-snapshot-nullifiers sapling-nullifiers.bin \
  --orchard-snapshot-nullifiers orchard-nullifiers.bin
```

This produces:

- `airdrop_configuration.json` — Contains Merkle roots for verification
- `sapling-nullifiers.bin` — Sapling pool snapshot nullifiers
- `orchard-nullifiers.bin` — Orchard pool snapshot nullifiers

> **For organizers**: Run this to generate the official Merkle roots and publish them along with the snapshot files.
>
> **For users**: Run this yourself to fetch the nullifiers, or download the snapshot files published by the airdrop organizers.

#### Snapshot File Format

The `.bin` snapshot files use a simple binary format:

- Each nullifier is **32 bytes** (raw binary)
- Nullifiers are concatenated sequentially with no header, delimiter, or padding
- Nullifiers must be **sorted in ascending lexicographic order**
- File size must be a multiple of 32 bytes

```
┌──────────────────┬──────────────────┬─────┬──────────────────┐
│  Nullifier 0     │  Nullifier 1     │ ... │  Nullifier N     │
│    (32 bytes)    │    (32 bytes)    │     │    (32 bytes)    │
└──────────────────┴──────────────────┴─────┴──────────────────┘
```

### Step 2: Obtain Your Viewing Keys

To scan for your notes, you need your Full Viewing Keys (FVKs). If you already have your viewing keys, you can skip to Step 3.

#### Viewing Key Format

The `airdrop` CLI expects viewing keys as **hex-encoded bytes**:

- **Orchard FVK**: 96 bytes → 192 hex characters
- **Sapling Diversifiable FVK**: 128 bytes → 256 hex characters

#### Helper Utility: `mnemonic-to-fvks`

If you don't have your viewing keys, the `mnemonic-to-fvks` utility can derive them from your wallet's mnemonic seed phrase:

```bash
mnemonic-to-fvks --network mainnet
```

The tool will securely prompt for:

- Your 24-word mnemonic
- Optional passphrase (press Enter if none)
- Account index (default: 0)

It outputs your Orchard and Sapling FVKs in the required hex format.

> **Security Note**: Keep your mnemonic secure. The FVKs themselves cannot spend funds but can reveal your transaction history.

### Step 3: Users Generate Their Claims

Download the snapshot files published by the airdrop organizer, then run `airdrop-claim` with your viewing keys:

```bash
airdrop airdrop-claim \
  --snapshot 419200..=2500000 \
  --lightwalletd-url https://lightwalletd.example.com \
  --sapling-snapshot-nullifiers sapling-nullifiers.bin \
  --orchard-snapshot-nullifiers orchard-nullifiers.bin \
  --sapling-fvk "zxviews1..." \
  --orchard-fvk "zxviewo1..." \
  --birthday-height 1900000 \
  --airdrop-claims-output-file my_claims.json
```

This command will:

1. Scan the blockchain for notes belonging to your viewing keys
2. For each unspent note found, generate a non-membership proof
3. Output the proofs to `my_claims.json`

**Parameters explained:**

| Parameter                       | Description                                                  |
| ------------------------------- | ------------------------------------------------------------ |
| `--snapshot`                    | Block height range for the airdrop snapshot                  |
| `--lightwalletd-url`            | URL of a lightwalletd server to scan the chain               |
| `--sapling-snapshot-nullifiers` | Path to the Sapling nullifiers snapshot file                 |
| `--orchard-snapshot-nullifiers` | Path to the Orchard nullifiers snapshot file                 |
| `--sapling-fvk`                 | Your Sapling Full Viewing Key                                |
| `--orchard-fvk`                 | Your Orchard Full Viewing Key                                |
| `--birthday-height`             | The block height when your wallet was created (optimization) |
| `--airdrop-claims-output-file`  | Output file for your claim proofs                            |

### Step 4: Submit Proofs

The output `my_claims.json` contains non-membership proofs that can be verified against the published Merkle roots—proving ownership of unspent shielded funds without revealing sensitive information.

## Privacy Properties

| Property                                         | Guaranteed |
| ------------------------------------------------ | ---------- |
| Proves you held unspent funds at snapshot height | Yes        |
| Reveals your actual nullifiers                   | No         |
| Reveals which specific notes you own             | No         |
| Requires spending or moving your funds           | No         |

## Environment Variables

Instead of passing arguments on the command line, you can use environment variables or a `.env` file:

| Variable           | Description                             |
| ------------------ | --------------------------------------- |
| `NETWORK`          | Network to use (`mainnet` or `testnet`) |
| `LIGHTWALLETD_URL` | Lightwalletd gRPC endpoint URL          |
| `SAPLING_FILE`     | Path to Sapling nullifiers file         |
| `ORCHARD_FILE`     | Path to Orchard nullifiers file         |

## Troubleshooting

### No notes found

- Verify your FVKs are correct using `mnemonic-to-fvks`
- Ensure your `--birthday-height` is at or before when you first received funds
- Check that you're connected to the correct network (`mainnet` vs `testnet`)

### Pool not active at snapshot height

Ensure your snapshot range starts after the pool activation height:

| Pool    | Network | Activation Height |
| ------- | ------- | ----------------- |
| Sapling | Mainnet | 419,200           |
| Sapling | Testnet | 280,000           |
| Orchard | Mainnet | 1,687,104         |
| Orchard | Testnet | 1,842,420         |

For example, to include Orchard notes on mainnet, your snapshot must start at or after block 1,687,104.
