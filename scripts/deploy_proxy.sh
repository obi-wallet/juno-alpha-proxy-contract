#!/bin/bash

if [ "$1" = "" ]
then
  echo "Usage: $0 1 arg required - local juno wallet with some coins for the 2 other multisig keys"
  exit
fi
MSIG1=$(junod keys show $1 --address)
# use some random numbers just to identify the wallets in local keychain
# later we might clean these up at end of script
let RAND1=$RANDOM*$RANDOM
let RAND2=$RAND1+1
MSIG_WALLET_NAME=multisigtest

# pinched and adapted from DA0DA0
IMAGE_TAG=${2:-"v9.0.0"}
CONTAINER_NAME="juno_obiproxy"
BINARY="junod"
DENOM='ujunox'
CHAIN_ID='uni-3'
RPC='https://rpc.uni.junomint.com:443/'
GAS1=--gas=auto
GAS2=--gas-prices=0.025ujunox
GAS3=--gas-adjustment=1.3

BLOCK_GAS_LIMIT=${GAS_LIMIT:-100000000} # should mirror mainnet

# create the other keys to be used in msig
echo "Adding new keys to wallet: $RAND1 and $RAND2"
echo "Storing wallet info in numbered plaintext files"
junod keys add $RAND1 > ./$RAND1.txt
MSIG2=$(grep -o '\bjuno\w*' $RAND1.txt)
junod keys add $RAND2 > ./$RAND2.txt
MSIG3=$(grep -o '\bjuno\w*' $RAND2.txt)

# fund the other accounts a little
junod tx bank send $1 $MSIG2 10000ujunox --fees 8000ujunox --chain-id=$CHAIN_ID --node=$RPC
# recommended that you wait between sends to avoid tx sequence mismatch
# TODO: mismatch handling
junod tx bank send $1 $MSIG3 10000ujunox --fees 8000ujunox --chain-id=$CHAIN_ID --node=$RPC

# legacy multisig. Note we can upgrade to whatever kinds of multisig later
# as wallets are proxy contracts
junod keys add $MSIG_WALLET_NAME --multisig-threshold 2 --multisig $1,$RAND1,$RAND2 > ./current_msig.txt
MSIGADDY=$(grep -o '\bjuno\w*' current_msig.txt)

echo "Building $IMAGE_TAG"
echo "Configured Block Gas Limit: $BLOCK_GAS_LIMIT"

# compile
docker run --rm -v "$(pwd)":/code \
  --mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  cosmwasm/rust-optimizer:0.12.5

# wallet addr
echo "Wallet to store contract: "
echo $MSIG1

BALANCE_1=$($BINARY q bank balances $MSIG1 --node=$RPC --chain-id=$CHAIN_ID)
echo "Pre-store balance for storer:"
echo $BALANCE_1

ADDRCHECK=$($BINARY keys show $MSIGADDY --address)
echo "Wallet to instantiate contract: $ADDRCHECK"

CONTRACT_CODE=$($BINARY tx wasm store "./artifacts/obi_proxy_contract.wasm" --from $1 --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3 --broadcast-mode block -y --output json | jq -r '.logs[0].events[-1].attributes[0].value')
echo "Stored: $CONTRACT_CODE"

# instantiate the CW721
OBIPROX_INIT="{\"admin\":\"$MSIGADDY\",\"hot_wallets\":[]})"

echo "$OBIPROX_INIT" | jq .
$BINARY tx wasm instantiate $CONTRACT_CODE $OBIPROX_INIT --from $MSIG_WALLET_NAME --node https://rpc.uni.junomint.com:443 --chain-id uni-3 --gas-prices 0.025ujunox --gas auto --gas-adjustment 1.3 --broadcast-mode block --output json --label "Obi Test Proxy" --admin $MSIGADDY
RES=$?

# get contract addr
CONTRACT_ADDRESS=$($BINARY q wasm list-contract-by-code $CONTRACT_CODE --output json | jq -r '.contracts[-1]')

echo $RES
exit $RES