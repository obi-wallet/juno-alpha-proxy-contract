#!/bin/bash
WALLET=scripttest

#import common vars and functions
source ./scripts/common.sh
source ./scripts/current_contract.sh

MSIG1=$($BINARY keys show $WALLET $KR --address)
MSIG_WALLET_NAME=multisigtest

echo -e "${YELLOW}Contract Optimization & Deployment Script${NC}"

echo "Adding new keys to wallet... "
RES=$($BINARY keys add signer1 $KR --no-backup > ./signer1.txt)
RES=$($BINARY keys add signer2 $KR --no-backup > ./signer2.txt)
MSIG2=$($BINARY keys show signer1 $KR --address)
MSIG3=$($BINARY keys show signer2 $KR --address)
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

MSIGADDY=$($BINARY keys show $MSIG_WALLET_NAME $KR --address)
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

CONTRACT_CODE=$($BINARY tx wasm store "./artifacts/obi_proxy_contract.wasm" $KR -y --from $WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3 --broadcast-mode block -y --output json | jq -r '.logs[0].events[-1].attributes[0].value')
echo "Contract code is $CONTRACT_CODE"

OBIPROX_INIT=$(jq -n --arg msigaddy $MSIG1 '{"admin":$msigaddy,"hot_wallets":[]}')
# test instantiate with just 1 address
RES=$($BINARY tx wasm instantiate $CONTRACT_CODE "$OBIPROX_INIT" $KR -y --from=$WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3 --output=json --label="Obi Test Proxy single" --admin=$MSIG1)
error_check "$RES" "Failed to instantiate contract"

echo ""
echo -n "Waiting 6 seconds for nodes to update... "
sleep 6s && echo " Done."
CONTRACT_ADDRESS=$($BINARY q wasm list-contract-by-code --node=$RPC --chain-id=$CHAIN_ID $CONTRACT_CODE --output json | jq -r '.contracts[-1]' 2>&1)
error_check $CONTRACT_ADDRESS "Failed to get contract address"
echo "Contract instantiated to $CONTRACT_ADDRESS."

HEADER="#!/bin/bash"
CODE="CONTRACT_CODE=$CONTRACT_CODE"
ADDY="CONTRACT_ADDRESS=$CONTRACT_ADDRESS"
ADMIN="CONTRACT_ADMIN_WALLET=$MSIG1"
printf "$HEADER\n$CODE\n$ADDY\n$ADMIN" > ./scripts/current_contract.sh
chmod +x ./scripts/current_contract.sh
echo "Updated current_contract.sh to include new values."
echo ""
bash ./scripts/test_contract.sh