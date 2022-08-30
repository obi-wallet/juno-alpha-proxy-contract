#!/bin/bash
GOROOT=/home/runner/goinstall/go
GOPATH=/home/runner/go
GO111MODULE=on
PATH=$PATH:/usr/local/goinstall/go/bin:/home/runner/go/bin

git clone https://github.com/CosmosContracts/juno
cd juno
git fetch
git checkout v6.0.0 #uni-3 testnet
make install