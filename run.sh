#!/usr/bin/env bash

echo X | sudo evtest |& grep $1 | grep -o ".*event[0-9]*" | xargs sudo ./target/debug/rst
