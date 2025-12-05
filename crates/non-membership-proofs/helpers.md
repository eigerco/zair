ls -lh orchard_nullifiers.bin sapling_nullifiers.bin

# Count nullifiers (file size / 32 bytes)

echo "Orchard: $(( $(stat -c%s orchard_nullifiers.bin) / 32 )) nullifiers"
echo "Sapling: $(( $(stat -c%s sapling_nullifiers.bin) / 32 )) nullifiers"

# View first 5 nullifiers as hex

echo "=== First 5 Orchard nullifiers ==="
head -c 160 orchard_nullifiers.bin | xxd -p -c 32

echo "=== First 5 Sapling nullifiers ==="
head -c 160 sapling_nullifiers.bin | xxd -p -c 32

# View last 5 nullifiers

echo "=== Last 5 Orchard nullifiers ==="
tail -c 160 orchard_nullifiers.bin | xxd -p -c 32

Or a one-liner to dump all as hex (careful with large files):

# Dump entire file as hex, one nullifier per line

xxd -p -c 32 orchard_nullifiers.bin | head -20
