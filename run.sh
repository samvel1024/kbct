#!/usr/bin/env bash

device=$1
laykeymap="/home/sme/.local/bin/laykeymap"

function get_file() {
  echo X | evtest 2>&1 | grep "$1" | grep -o ".*event[0-9]*" || true
}

function try_run_mapper() {
  file=$(get_file "$device")
  if [ -z "$file" ]; then
    echo "Device not found in /dev/input, waiting..."
  else
    echo "Running mapper for device file ${file}"
    ${laykeymap} "${file}"
    echo "Mapper exited, waiting..."
  fi
}

function watch() {
  while [ true ]; do
    try_run_mapper "$device"
    inotifywait -e create /dev/input
  done
}

watch
