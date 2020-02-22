#!/usr/bin/bash

set -e
git submodule update --init --recursive
mkdir -p build
bash -c "cd build && cmake .. && make"

echo "BUILD SUCCESS"
echo "For examples refer to https://github.com/samvel1024/layketmap"
