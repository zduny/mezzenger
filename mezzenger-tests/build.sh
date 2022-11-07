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

echo "\nBuilding worker..."
cd worker
wasm-pack build --release --target no-modules
cd ..

echo "\nBuilding client..."
cd client
wasm-pack build --release --target web
cd ..

echo "\nBuilding native client..."
cd client-native
cargo build --release
cd ..

cp worker/pkg/worker.js www/worker_wasm.js
cp worker/pkg/worker_bg.wasm www/worker_wasm_bg.wasm

cp client/pkg/client.js www/client.js
cp client/pkg/client_bg.wasm www/client_bg.wasm

cp server/target/release/server ./tests_server
cp client-native/target/release/client-native ./tests_client
