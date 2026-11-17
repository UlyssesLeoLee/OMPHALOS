#!/usr/bin/env sh
set -eu
cargo install --path . --force
printf '\nSet requirements paths in PROJECT/.concept-guard/config.toml, then run:\n  concept-guard --root PROJECT init\n  concept-guard --root PROJECT approve\n'
