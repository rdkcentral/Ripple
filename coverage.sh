#!/bin/sh
#
# This script is very based on .github/workflows/Build.yml
INSTALLED=$(cargo install --list | grep cargo-llvm-cov)
if [ -z "$INSTALLED" ]; then
    echo "Installing cargo-llvm-cov"
    cargo install cargo-llvm-cov
fi
LOWER_COVERAGE_THRESHOLD=$(cat ./ci/coverage_threshold.txt |  cut -d ' ' -f1)

cargo llvm-cov --cobertura --output-path coverage.cobertura.xml --ignore-filename-regex ".*[\\/](.*distributor*)[\\/]?.*"
CURRENT_COVERAGE=$(grep '<coverage' coverage.cobertura.xml | grep -o 'line-rate="[0-9.]\+"' | grep -o '[0-9.]\+')
CURRENT_COVERAGE=$(printf %.0f $(echo "$CURRENT_COVERAGE*100" | bc))

echo "Current coverage: $CURRENT_COVERAGE, allowed coverage: $LOWER_COVERAGE_THRESHOLD"
