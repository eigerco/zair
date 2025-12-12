# zcash-namada-airdrop

A playground for test-driving the Zcash Namada airdrop implementation.

## Generic instructions

This workspace is using Nix to enhance development experience.

- **`nix develop`** to enter the development environment.
- **`nix fmt`** to format the workspace
- **`nix flake check`** to run checks, like linters and formatters.

The workspace is also using pre-commit checks. These can be removed if they prove problematic.

## Available Tools

### mnemonic-to-fvks

- **Description**: A utility to convert a Zcash mnemonic to Full Viewing Keys. Supports Orchard and Sapling pools. Run with `--help` to check the usage.

### zcash-notes-proof

- **Description**: Searches the Zcash network for user notes. It uses the `lightwalletd` protocol to search for the notes. Run with `--help` to check the usage. The `zcash-notes-proof` is using the GRPC client to connect with `lightwalletd` and queries the chain.

### TODO

Describe all the tools present in the PR

## Zcash pools

| Pool    | Network | Enabled at Block Height |
| ------- | ------- | ----------------------- |
| Sapling | Mainnet | 419,200                 |
| Sapling | Testnet | 280,000                 |
| Orchard | Mainnet | 1,687,104               |
| Orchard | Testnet | 1,842,420               |

## TODO

Check the [zaino](https://github.com/zingolabs/zaino)

RUSTDOCFLAGS="--html-in-header $(pwd)/crates/non-membership-proofs/katex.html" cargo doc -p non-membership-proofs --no-deps --open

## Setup Instructions

After you clone the repo

### Without nix

```bash
git submodule update --init --recursive

git clone --branch v0.11.0 --single-branch https://github.com/zcash/orchard.git .patched-orchard
git -C .patched-orchard apply "../nix/airdrop-orchard-nullifier.patch"

git clone --branch v0.5.0 --single-branch https://github.com/zcash/sapling-crypto.git .patched-sapling-crypto
git -C .patched-sapling-crypto apply "../nix/airdrop-sapling-nullifier.patch"
```

### With nix

```bash
git submodule update --init --recursive

nix develop
```
