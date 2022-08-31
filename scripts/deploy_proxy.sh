#!/bin/bash
WALLET=scripttest

#import common vars and functions
source ./scripts/common.sh
source ./scripts/current_contract.sh

MSIG1=$($BINARY keys show $WALLET $KR --address)
MSIG_WALLET_NAME=multisigtest

echo -e "${YELLOW}Contract Optimization & Deployment Script${NC}"

if [[ $AUTOYES == 1 ]]
then
  REPLY=y
fi
if [[ $AUTOYES == 0 ]]
then
  read -p "Generate and fund new msig autokeys (n to use existing keys from previous run)? " -n 1 -r
  echo    # (optional) move to a new line
fi
if [[ $REPLY =~ ^[Yy]$ ]]
then
  # use some random numbers just to identify the new wallets in local keychain
  # later we might clean these up at end of script
  RES=$($BINARY keys delete $MSIG_WALLET_NAME $KR -y -f 2>&1)
  # error_check here would throw if the key didn't already exist,
  # which we don't care about
  let RAND1=$RANDOM*$RANDOM
  let RAND2=$RAND1+1

  # create the other keys to be used in msig
  echo "Adding new keys to wallet: autokey1.txt and autokey2.txt... "
  RES=$($BINARY keys add $RAND1 $KR --no-backup > ./autokey1.txt)
  RES=$($BINARY keys add $RAND2 $KR --no-backup > ./autokey2.txt)
  MSIG2=$(grep -o '\bjuno\w*' ./autokey1.txt)
  MSIG3=$(grep -o '\bjuno\w*' ./autokey2.txt)
  # fund the other accounts a little
  RES=$($BINARY tx bank send $WALLET $MSIG2 10000$DENOM $KR -y --fees 5000$DENOM --chain-id=$CHAIN_ID --node=$RPC 2>&1)
  error_check "$RES" "Funding msig2 account $MSIG2 from $WALLET failed"
  # recommended that you wait between sends to avoid tx sequence mismatch
  # TODO: mismatch handling
  echo "Funded msig2 signer $MSIG2."
  echo -n "Waiting 10 seconds to fund msig3 signer $MSIG3..."
  sleep 10s && echo " Done."
  RES=$($BINARY tx bank send $WALLET $MSIG3 10000$DENOM $KR -y --fees 5000$DENOM --chain-id=$CHAIN_ID --node=$RPC 2>&1)
  error_check "$RES" "Funding msig3 account $MSIG3 from $WALLET failed"

  # ... aaaand in this implementation the other keys
  # need to also transact so that pubkeys are on chain.
  # Conveniently we return some testnet juno.
  echo -n "Waiting 6 seconds for nodes to update... "
  sleep 6s && echo " Done."
  echo -n "Now activating msig signers on-chain by sending some funds back..."
  RES=$($BINARY tx bank send $MSIG2 $MSIG1 4000$DENOM $KR -y --fees 5000$DENOM --chain-id=$CHAIN_ID --node=$RPC)
  error_check "$RES" "Sending from $MSIG2 back to $WALLET ($MSIG1) failed"
  RES=$($BINARY tx bank send $MSIG3 $MSIG1 4000$DENOM $KR -y --fees 5000$DENOM --chain-id=$CHAIN_ID --node=$RPC)
  error_check "$RES" "Sending from $MSIG3 back to $WALLET ($MSIG1) failed"
  echo " Done."
  # legacy multisig. Note we can upgrade to whatever kinds of multisig later
  # as wallets are proxy contracts
  # note that --no-sort is omitted so order doesn't matter
  echo ""
  echo -n "Creating new multisig..."
  $BINARY keys add $MSIG_WALLET_NAME $KR --multisig-threshold 2 --multisig $WALLET,$RAND1,$RAND2 > ./current_msig.txt
else
  MSIG2=$(grep -o '\bjuno\w*' ./autokey1.txt)
  MSIG3=$(grep -o '\bjuno\w*' ./autokey2.txt)
fi

MSIGADDY=$(grep -o '\bjuno\w*' ./current_msig.txt)
echo "Multisig address is $MSIGADDY. Stored in ./current_msig.txt"
echo ""
echo -n "Waiting 6 seconds to avoid sequence mismatch..."
sleep 6s && echo " Done."
# fund the multisig so it can deploy
echo "Funding the multisig address itself..."
RES=$($BINARY tx bank send $WALLET $MSIGADDY 500000$DENOM $KR -y --fees 5000$DENOM --chain-id=$CHAIN_ID --node=$RPC)
error_check "$RES" "Funding multisig address failed"

echo "Using multisig address: $MSIGADDY. Address saved in ./current_msig.txt."

echo ""
echo -n "Optimizing smart contract code..."
# compile
RES=$(docker run --rm -v "$(pwd)":/code \
  --mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  cosmwasm/rust-optimizer:0.12.5)
echo " Done."

# wallet addr
echo "Wallet to store contract: "
echo $MSIG1

BALANCE_1=$($BINARY q bank balances $MSIG1 --node=$RPC --chain-id=$CHAIN_ID 2>&1)
error_check BALANCE_1 "Failed to get balance for $MSIG1"
echo "Pre-store balance for storer:"
echo $BALANCE_1

ADDRCHECK=$($BINARY keys show $MSIGADDY $KR --address 2>&1)
error_check ADDRCHECK "Failed to verify address to instantiate contract"
echo "Wallet to instantiate contract: $ADDRCHECK"
echo "NOTE: for simplicity, the admin will just be a single signer for now."

# store the contract code
echo "Contract code currently stored at $CONTRACT_CODE."
if [[ $AUTOYES == 1 ]]
then
  REPLY=y
fi
if [[ $AUTOYES == 0 ]]
then
  echo "Would you like to store updated contract code? This makes sense"
  echo "if there have been some contract updates."
  read -p "Store updated code? (n to use existing code) " -n 1 -r
  echo    # (optional) move to a new line
fi
if [[ $REPLY =~ ^[Yy]$ ]]
then
  CONTRACT_CODE=$($BINARY tx wasm store "./artifacts/obi_proxy_contract.wasm" $KR -y --from $WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3 --broadcast-mode block -y --output json | jq -r '.logs[0].events[-1].attributes[0].value')
  echo "Contract code is $CONTRACT_CODE"
fi

OBIPROX_INIT=$(jq -n --arg msigaddy $MSIG1 '{"admin":$msigaddy,"hot_wallets":[]}')
# test instantiate with just 1 address
RES=$($BINARY tx wasm instantiate $CONTRACT_CODE "$OBIPROX_INIT" $KR -y --from=$WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3 --output=json --label="Obi Test Proxy single" --admin=$MSIG1)
error_check "$RES" "Failed to instantiate contract"

# instantiate the contract with multiple signers
# generate the tx for others to sign with --generate-only
# rm -rf ./tx_to_sign.txt ./partial_tx_1.json ./partial_tx_2.json ./completed_tx.json
# $BINARY tx wasm instantiate $CONTRACT_CODE "$OBIPROX_INIT" --generate-only --from=$MSIGADDY --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3 --output=json --label="Obi Test Proxy" --admin=$MSIGADDY > ./tx_to_sign.json
# echo "Transaction to sign in ./tx_to_sign.json"

# sign with each address
# $BINARY tx sign ./tx_to_sign.txt --multisig=$MSIGADDY --from=$WALLET --sign-mode=amino-json --node=$RPC --chain-id=$CHAIN_ID --output-document=sig1.json
# $BINARY tx sign ./tx_to_sign.txt --multisig=$MSIGADDY --from=$RAND1 --sign-mode=amino-json --node=$RPC --chain-id=$CHAIN_ID --output-document=sig2.json
# $BINARY tx sign ./tx_to_sign.txt --multisig=$MSIGADDY --from=$RAND2 --sign-mode=amino-json --node=$RPC --chain-id=$CHAIN_ID --output-document=sig3.json
# we only need 2 of the 3 though
# $BINARY tx multisign ./tx_to_sign.json $MSIG_WALLET_NAME sig1.json sig2.json > ./completed_tx.json
# $BINARY tx broadcast ./completed_tx.json

# get contract addr
echo ""
echo -n "Waiting 6 seconds for nodes to update... "
sleep 6s && echo " Done."
CONTRACT_ADDRESS=$($BINARY q wasm list-contract-by-code --node=$RPC --chain-id=$CHAIN_ID $CONTRACT_CODE --output json | jq -r '.contracts[-1]' 2>&1)
error_check $CONTRACT_ADDRESS "Failed to get contract address"
echo "Contract instantiated to $CONTRACT_ADDRESS."

rm -rf ./scripts/current_contract.sh
HEADER="#!/bin/bash"
CODE="CONTRACT_CODE=$CONTRACT_CODE"
ADDY="CONTRACT_ADDRESS=$CONTRACT_ADDRESS"
ADMIN="CONTRACT_ADMIN_WALLET=$MSIG1"
printf "$HEADER\n$CODE\n$ADDY\n$ADMIN" > ./scripts/current_contract.sh
chmod +x ./scripts/current_contract.sh
echo "Updated current_contract.sh to include new values."
echo ""
if [[ $AUTOYES == 1 ]]
then
  REPLY=y
  echo "Proceeding to single-key admin contract tests."
fi
if [[ $AUTOYES == 0 ]]
then
  read -p "You're now ready for single-key admin contract tests. Proceed? " -n 1 -r
  echo
fi
if [[ $REPLY =~ ^[Yy]$ ]]
then
  bash ./scripts/test_contract.sh
fi