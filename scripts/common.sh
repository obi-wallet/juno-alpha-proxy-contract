#!/bin/bash

BINARY="junod"
DENOM='ujunox'
CHAIN_ID='uni-3'
RPC='https://rpc.uni.juno.deuslabs.fi:443'
GAS1=--gas=auto
GAS2=--gas-prices=0.025ujunox
GAS3=--gas-adjustment=1.3
KR=--keyring-backend=test

error_check () {
  if [[ $1 = *NotFound* ]]
  then
    echo "$2: sending account does not exist yet"
    exit 1
  fi
  if [[ $1 = *mismatch* ]]
  then
    echo "$2: sequence mismatch"
    exit 1
  fi
  if [[ $1 = *verify* ]]
  then
    echo "$2: signature verification failed"
    exit 1
  fi
  if [[ $1 = *nsufficient* ]]
  then
    echo "$2: not enough funds"
    exit 1
  fi
  if [[ $1 = *Unauthorized* ]]
  then
    echo "$2: not authorized as contract admin (Unauthorized)"
    exit 1
  fi
  if [[ $1 = *Usage:* ]]
  then
    echo "$2: malformed commmand"
    exit 1
  fi
}
