#!/bin/sh
set -eu

runtime_dir=$(CDPATH= cd -- "$(dirname "$0")/container-runtime" && pwd)
(
  cd "$runtime_dir"
  npm ci --ignore-scripts
  node test-events.mjs
)
