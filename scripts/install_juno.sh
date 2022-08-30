#!/bin/bash
export GOROOT=/home/runner/goinstall/go
export GOPATH=/home/runner/go
export GO111MODULE=on
export PATH=$PATH:/usr/local/goinstall/go/bin:/home/runner/go/bin

wget https://www.dropbox.com/s/44fig40y3825g8u/junod?dl=1
mv ./junod?dl=1 ./junod
chmod +x ./junod