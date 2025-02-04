#!/bin/sh
INSTALLED=$(cargo install --list | grep cargo-llvm-cov)
if [ -z "$INSTALLED" ]; then
    echo "Installing cargo-llvm-cov"
    cargo install cargo-llvm-cov
fi


cargo llvm-cov --cobertura --output-path coverage.cobertura.xml --ignore-filename-regex ".*[\\/](.*distributor*)[\\/]?.*"
CURRENT_COVERAGE=$(grep '<coverage' coverage.cobertura.xml | grep -o 'line-rate="[0-9.]\+"' | grep -o '[0-9.]\+')
echo "Current coverage: $CURRENT_COVERAGE"
