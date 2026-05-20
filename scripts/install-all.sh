#!/usr/bin/env bash

set -euo pipefail

cargo install --path backlight --locked
cargo install --path luksctl --locked
cargo install --path notifi --locked
cargo install --path search --locked
cargo install --path wallpaper --locked
cargo install --path writeback --locked