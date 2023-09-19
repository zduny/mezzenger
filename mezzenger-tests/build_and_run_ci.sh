#!/bin/sh
set -e

echo "Building..."
./build.sh

echo "\nRunning..."
./run_ci.sh
