#!/bin/bash

DUMMYPRICECONTRACT="juno1u6ywzgf3kkydyfm0k3y4gwj5gsz42lpvgnfpl3fj8vfgcxy4cffqe4en4h"
BINARY="./junod"
# These should be provided by environment
# DENOM='ujunox'
# CHAIN_ID='uni-3'
# RPC='https://rpc.uni.juno.deuslabs.fi:443'
GAS1=--gas=auto
GAS2="--gas-prices=0.025$DENOM"
GAS3=--gas-adjustment=1.3
KR=--keyring-backend=test
RED='\033[0;31m'
LBLUE='\033[1;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

error_check () {
  if [[ $4 != "" && $1 == *"$4"* ]];
  then
    echo "Received alternate error: $4 ✅"
    return 0;
  fi
  if [[ $3 != "" && $1 == *"$3"* ]];
  then
    echo "Received expected error: $3 ✅"
    return 0;
  fi
  if [[ $1 == *"not found: key not found"* ]]
  then
    echo "$2: sending account does not exist yet"
    exit 1
  fi
  if [[ $1 == *"sequence mismatch"* ]]
  then
    echo "$2: sequence mismatch. Try acting less quickly."
    exit 1
  fi
  if [[ $1 == *"signature verification failed"* ]]
  then
    echo "$2: signature verification failed"
    exit 1
  fi
  if [[ $1 == *"insufficient funds"* ]]
  then
    echo "$2: not enough funds to pay for fees"
    exit 1
  fi
  if [[ $1 == *"Caller is not admin"* ]]
  then
    echo "$2: Caller is not admin"
    exit 1
  fi
  if [[ $1 == *"Spend-limited cw20 transactions cannot have additional funds attached"* ]]
  then
    echo "$2: Spend-limited cw20 transactions cannot have additional funds attached"
    exit 1
  fi
  if [[ $1 == *"Spend-limited WasmMsg txes must be cw20 Send or Transfer messages"* ]]
  then
    echo "$2: Spend-limited WasmMsg txes must be cw20 Send or Transfer messages"
    exit 1
  fi
  if [[ $1 == *"Message deserialization error.  Spend-limited WasmMsg txes are limited to a Cw20ExecuteMsg Send or Transfer"* ]]
  then
    echo "$2: Message deserialization error.  Spend-limited WasmMsg txes are limited to a Cw20ExecuteMsg Send or Transfer"
    exit 1
  fi
  if [[ $1 == *"WASM message is not Execute. Spend-limited WasmMsg txes are limited to a Cw20ExecuteMsg Send or Transfer"* ]]
  then
    echo "$2: WASM message is not Execute. Spend-limited WasmMsg txes are limited to a Cw20ExecuteMsg Send or Transfer"
    exit 1
  fi
  if [[ $1 == *"This address is not permitted to spend this token, or to spend this many of this token"* ]]
  then
    echo "$2: This address is not permitted to spend this token, or to spend this many of this token"
    exit 1
  fi
  if [[ $1 == *"Spend-limited transactions must be BankMsg or WasmMsg (Cw20ExecuteMsg Send or Transfer)"* ]]
  then
    echo "$2: Spend-limited transactions must be BankMsg or WasmMsg (Cw20ExecuteMsg Send or Transfer)"
    exit 1
  fi
  if [[ $1 == *"This address is already authorized as a Hot Wallet. Remove it first in order to update it"* ]]
  then
    echo "$2: This address is already authorized as a Hot Wallet. Remove it first in order to update it"
    exit 1
  fi
  if [[ $1 == *"This address is not authorized as a spend limit Hot Wallet"* ]]
  then
    echo "$2: This address is not authorized as a spend limit Hot Wallet"
    exit 1
  fi
  if [[ $1 == *"Failed to advance the reset day"* ]]
  then
    echo "$2: Failed to advance the reset day"
    exit 1
  fi
  if [[ $1 == *"Failed to advance the reset month"* ]]
  then
    echo "$2: Failed to advance the reset month"
    exit 1
  fi
  if [[ $1 == *"Hot wallet does not have a spend limit for this asset"* ]]
  then
    echo "$2: Hot wallet does not have a spend limit for asset"
    exit 1
  fi
  if [[ $1 == *"You cannot spend more than your available spend limit"* ]]
  then
    echo "$2: You cannot spend more than your available spend limit"
    exit 1
  fi
  if [[ $1 == *"Uninitialized message"* ]]
  then
    echo "$2: Uninitialized message"
    exit 1
  fi
  if [[ $1 == *"Caller is not pending new admin. Propose new admin first"* ]]
  then
    echo "$2: Caller is not pending new admin. Propose new admin first"
    exit 1
  fi
  if [[ $1 == *"Unauthorized"* ]]
  then
    echo "$2: not authorized as contract admin (Unauthorized)"
    exit 1
  fi
  if [[ $1 == *"Usage: junod"* ]]
  then
    echo "$2: other error or malformed commmand"
    exit 1
  fi
  if [[ $1 == *"Error:"* ]]
  then
    echo "$2: other error or malformed commmand"
    echo "****DUMP****:"
    echo -n -e "${YELLOW}"
    echo "$1"
    echo -e "${NC}"
    exit 1
  fi
  echo " Done ✅"
  echo "$1" | /usr/bin/grep -w "txhash"
}
