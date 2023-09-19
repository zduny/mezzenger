#!/bin/sh
set -e

sleep 1 && chrome http://localhost:3030 &
sleep 1 && ./tests_client &
./tests_server
