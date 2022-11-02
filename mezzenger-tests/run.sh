#!/bin/sh
set -e

sleep 1 && open http://localhost:3030 &
./tests_server
