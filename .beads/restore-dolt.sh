#!/bin/bash
cd /home/lewis/gt/veloxide/polecats/vault/veloxide/.beads/dolt
git fetch origin
git reset --hard origin/main
git submodule update --init --recursive
