#!/bin/bash

set -v
echo "start to run cargo clippy"
cargo clippy --fix --allow-staged >> output.txt
if [ $? -ne 0 ] 
    then
    echo "cargo clippy failed with warnings or errors"
        exit 1
    fi
echo "cargo clippy is successfully executed"