#!/bin/bash

mkdir -p target/deploy

count=0
goal=1

while [ $count -lt $goal ]; do
    keyfile="target/deploy/temp-keypair.json"
    solana-keygen new --no-bip39-passphrase --outfile "$keyfile" --force > /dev/null 2>&1
    pubkey=$(solana-keygen pubkey "$keyfile")

    echo -n "."  # 👈 Это будет печатать точку при каждой попытке

    if [[ $pubkey == Rou* ]]; then
        echo
        echo "✅ Found: $pubkey"
        cp "$keyfile" "target/deploy/vanity_$count-keypair.json"
        ((count++))
    fi
done

echo "🎉 $count vanity key(s) saved in target/deploy/"
