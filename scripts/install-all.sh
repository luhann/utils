#!/usr/bin/env bash

set -euo pipefail

cargo install --path bin/backlight --locked
cargo install --path bin/luksctl --locked
cargo install --path bin/notifi --locked
cargo install --path bin/search --locked
cargo install --path bin/wallpaper --locked
cargo install --path bin/writeback --locked
