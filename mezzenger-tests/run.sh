#!/bin/sh
set -e

sleep 1 && open http://localhost:3030 &
sleep 1 && ./tests_client &
./tests_server
