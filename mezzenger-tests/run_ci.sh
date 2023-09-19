#!/bin/sh
set -e

sleep 1 && chrome --headless --enable-logging=stderr --v=1 --disable-gpu http://localhost:3030 &
sleep 1 && ./tests_client &
./tests_server
