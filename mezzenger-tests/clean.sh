#!/bin/sh
set -e

cd common
cargo clean
cd ..

cd server
cargo clean
cd ..

cd worker
cargo clean
rm -rf pkg
cd ..

cd client
cargo clean
rm -rf pkg
cd ..

cd client-native
cargo clean
cd ..

cd www
rm -f www/worker_wasm.js
rm -f www/worker_wasm_bg.wasm
rm -f client.js
rm -f client_bg.wasm
cd ..

rm -f tests_server
