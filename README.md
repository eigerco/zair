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
