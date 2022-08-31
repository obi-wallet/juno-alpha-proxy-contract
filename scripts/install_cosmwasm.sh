#!/bin/bash
wget https://www.dropbox.com/s/e17oj0hbiv3wx7w/cosmwasm.tar.gz?dl=0
mv ./cosmwasm.tar.gz?dl=0 ./cosmwasm.tar.gz
mkdir -p $GOPATH/pkg/mod/github.com
tar xvf ./cosmwasm.tar.gz -C $GOPATH/pkg/mod/github.com
