#!/bin/sh

while true; do
    target/release/slothunter -c configuration-template.toml > log 2>&1 || true
    sleep 1
done
