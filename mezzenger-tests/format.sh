#!/bin/sh
set -e

echo "Formatting common..."
cd common
cargo fmt
cd ..

echo "Formatting server..."
cd server
cargo fmt
cd ..

echo "Formatting worker..."
cd worker
cargo fmt
cd ..

echo "Formatting client..."
cd client
cargo fmt
cd ..

echo "Done."
