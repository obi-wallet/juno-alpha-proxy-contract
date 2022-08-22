#!/bin/bash

if [ "$1" = "" ]
then
  echo "Usage: $0 1 arg required - local juno wallet with some coins for the 2 other multisig keys"
  exit
fi

#import common vars
source ./scripts/common.sh
source ./scripts/current_contract.sh

MSIG1=$($BINARY keys show $1 --address)
MSIG_WALLET_NAME=multisigtest

read -p "Generate and fund new msig autokeys (n to use existing keys from previous run)? " -n 1 -r
echo    # (optional) move to a new line
if [[ $REPLY =~ ^[Yy]$ ]]
then
  # use some random numbers just to identify the new wallets in local keychain
  # later we might clean these up at end of script
  let RAND1=$RANDOM*$RANDOM
  let RAND2=$RAND1+1
  mv -f ./autokey1.txt ./backup_autokey$RAND1.txt
  mv -f ./autokey2.txt ./backup_autokey$RAND2.txt

  # create the other keys to be used in msig
  echo "Adding new keys to wallet: autokey1.txt and autokey2.txt"
  $BINARY keys add $RAND1 > ./autokey1.txt
  $BINARY keys add $RAND2 > ./autokey2.txt
  MSIG2=$(grep -o '\bjuno\w*' ./autokey1.txt)
  MSIG3=$(grep -o '\bjuno\w*' ./autokey2.txt)
  # fund the other accounts a little
  $BINARY tx bank send $1 $MSIG2 10000$DENOM --fees 5000$DENOM --chain-id=$CHAIN_ID --node=$RPC
  # recommended that you wait between sends to avoid tx sequence mismatch
  # TODO: mismatch handling
  $BINARY tx bank send $1 $MSIG3 10000$DENOM --fees 5000$DENOM --chain-id=$CHAIN_ID --node=$RPC

  # ... aaaand in this implementation the other keys
  # need to also transact so that pubkeys are on chain.
  # Conveniently we return some testnet juno.
  $BINARY tx bank send $MSIG2 $MSIG1 4000$DENOM --fees 5000$DENOM --chain-id=$CHAIN_ID --node=$RPC
  $BINARY tx bank send $MSIG3 $MSIG1 4000$DENOM --fees 5000$DENOM --chain-id=$CHAIN_ID --node=$RPC

  # legacy multisig. Note we can upgrade to whatever kinds of multisig later
  # as wallets are proxy contracts
  # note that --no-sort is omitted so order doesn't matter
  $BINARY keys add $MSIG_WALLET_NAME --multisig-threshold 2 --multisig $1,$RAND1,$RAND2 > ./current_msig.txt
else
  MSIG2=$(grep -o '\bjuno\w*' ./autokey1.txt)
  MSIG3=$(grep -o '\bjuno\w*' ./autokey2.txt)
fi

MSIGADDY=$(grep -o '\bjuno\w*' ./current_msig.txt)

# fund the multisig so it can deploy
$BINARY tx bank send $1 $MSIGADDY 500000$DENOM --fees 5000$DENOM --chain-id=$CHAIN_ID --node=$RPC

echo "Using multisig address: $MSIGADDY. Address saved in ./current_msig.txt."

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

# store the contract code
echo "Contract code currently stored at $CONTRACT_CODE."
echo "Would you like to store updated contract code? This makes sense"
echo "if there have been some contract updates."
read -p "Store updated code? (n to use existing code) " -n 1 -r
echo    # (optional) move to a new line
if [[ $REPLY =~ ^[Yy]$ ]]
then
  CONTRACT_CODE=$($BINARY tx wasm store "./artifacts/obi_proxy_contract.wasm" --from $1 --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3 --broadcast-mode block -y --output json | jq -r '.logs[0].events[-1].attributes[0].value')
fi
echo "Contract code is $CONTRACT_CODE"

OBIPROX_INIT=$(jq -n --arg msigaddy $MSIG1 '{"admin":$msigaddy,"hot_wallets":[]}')
# test instantiate with just 1 address
$BINARY tx wasm instantiate $CONTRACT_CODE "$OBIPROX_INIT" --from=$1 --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3 --output=json --label="Obi Test Proxy single" --admin=$MSIG1

# instantiate the contract with multiple signers
# generate the tx for others to sign with --generate-only
# rm -rf ./tx_to_sign.txt ./partial_tx_1.json ./partial_tx_2.json ./completed_tx.json
# $BINARY tx wasm instantiate $CONTRACT_CODE "$OBIPROX_INIT" --generate-only --from=$MSIGADDY --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3 --output=json --label="Obi Test Proxy" --admin=$MSIGADDY > ./tx_to_sign.json
# echo "Transaction to sign in ./tx_to_sign.json"

# sign with each address
# $BINARY tx sign ./tx_to_sign.txt --multisig=$MSIGADDY --from=$1 --sign-mode=amino-json --node=$RPC --chain-id=$CHAIN_ID --output-document=sig1.json
# $BINARY tx sign ./tx_to_sign.txt --multisig=$MSIGADDY --from=$RAND1 --sign-mode=amino-json --node=$RPC --chain-id=$CHAIN_ID --output-document=sig2.json
# $BINARY tx sign ./tx_to_sign.txt --multisig=$MSIGADDY --from=$RAND2 --sign-mode=amino-json --node=$RPC --chain-id=$CHAIN_ID --output-document=sig3.json
# we only need 2 of the 3 though
# $BINARY tx multisign ./tx_to_sign.json $MSIG_WALLET_NAME sig1.json sig2.json > ./completed_tx.json
# $BINARY tx broadcast ./completed_tx.json

# get contract addr
CONTRACT_ADDRESS=$($BINARY q wasm list-contract-by-code --node=$RPC --chain-id=$CHAIN_ID $CONTRACT_CODE --output json | jq -r '.contracts[-1]')
echo "Contract instantiated to $CONTRACT_ADDRESS."

rm -rf ./scripts/current_contract.sh
HEADER="#!/bin/bash"
CODE="CONTRACT_CODE=$CONTRACT_CODE"
ADDY="CONTRACT_ADDRESS=$CONTRACT_ADDRESS"
ADMIN="CONTRACT_ADMIN_WALLET=$MSIG1"
printf "$HEADER\n$CODE\n$ADDY\n$ADMIN" > ./scripts/current_contract.sh
chmod +x ./scripts/current_contract.sh
echo "Updated current_contract script."
echo
read -p "You're now ready for single-key admin contract tests. Proceed? " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]
then
  ./scripts/test_contract.sh
fi