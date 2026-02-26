# Getting Started

This section covers setting up the repository, building `zair`, and running a quick sanity check. Afterwards, follow the step-by-step guide in the [CLI Reference](../cli/index.md).

## Setup and Building

### With Nix (recommended)

The repo includes a Nix flake that provides all dependencies (Rust, `protoc`, patched crates):

```bash
nix develop
cargo build --release
```

### Without Nix

#### Prerequisites

- Rust 1.91+ (2024 edition)
- Protobuf (`protoc`) for lightwalletd gRPC bindings

#### Patching dependencies

The airdrop circuits require patched versions of upstream Zcash crates. After cloning, run:

```bash
git clone --branch v0.11.0 --single-branch https://github.com/zcash/orchard.git .patched-orchard
git -C .patched-orchard apply "../nix/airdrop-orchard-nullifier.patch"

git clone --branch v0.5.0 --single-branch https://github.com/zcash/sapling-crypto.git .patched-sapling-crypto
git -C .patched-sapling-crypto apply "../nix/airdrop-sapling-nullifier.patch"

curl -sL https://static.crates.io/crates/halo2_gadgets/halo2_gadgets-0.3.1.crate | tar xz
mv halo2_gadgets-0.3.1 .patched-halo2-gadgets
patch -p1 -d .patched-halo2-gadgets < nix/airdrop-halo2-gadgets-sha256.patch
```

The patches mainly expose private internals needed by the airdrop circuits.

Then build:

```bash
cargo build --release
```

## Sanity check

Verify the CLI is available:

```bash
./target/release/zair --help
```

and inspect the command groups:

```bash
./target/release/zair key --help
./target/release/zair setup --help
./target/release/zair config --help
./target/release/zair claim --help
./target/release/zair verify --help
```

## Feature flags

The proving pipeline is gated behind the `prove` feature for some crates/binaries, enabled by default.

If you only need verification, you may build without proving support for a lighter dependency.
