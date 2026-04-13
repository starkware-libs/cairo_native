#!/usr/bin/env bash

# Script to check the corelib version matches.

_result=$(grep "version = \"$1\"" corelib/Scarb.toml)

if [ $? -ne 0 ]; then
  echo "corelib version mismatch, please update the vendor/cairo submodule:"
  echo "- git submodule update --init"
  exit 1
fi
