#!/bin/bash

git clone https://github.com/CosmosContracts/juno
cd juno
git fetch
git checkout v6.0.0 #uni-3 testnet
make install