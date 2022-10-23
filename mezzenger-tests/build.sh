#!/bin/sh
set -e

echo "Building common..."
cd common
cargo build --release
cd ..

echo "\nBuilding server..."
cd server
cargo build --release
cd ..

echo "\nBuilding client..."
cd client
wasm-pack build --release --target web
cd ..
