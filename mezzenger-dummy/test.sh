#!/bin/sh
set -e

echo "Testing in tokio..."
cargo test

echo "\nTesting in browser..."
wasm-pack test --chrome --headless
