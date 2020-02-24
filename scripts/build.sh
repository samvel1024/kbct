#!/usr/bin/env bash

set -e
git submodule update --remote
mkdir -p build
cd build
cmake .. && make