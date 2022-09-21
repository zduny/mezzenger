#!/bin/sh
set -e

echo "Testing on tokio..."
cargo test

echo "\nTesting on browser..."
wasm-pack test --chrome --headless
