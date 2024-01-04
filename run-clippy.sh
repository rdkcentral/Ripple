#!/bin/bash

cargo fmt --all &&
cargo clippy --tests --examples --all-targets --all-features -- -D warnings -A clippy::large_enum_variant
