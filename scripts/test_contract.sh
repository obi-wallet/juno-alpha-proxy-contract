#!/bin/bash
BINARY="junod"
DENOM='ujunox'
CHAIN_ID='uni-3'
RPC='https://rpc.uni.juno.deuslabs.fi:443'
GAS1=--gas=auto
GAS2=--gas-prices=0.025ujunox
GAS3=--gas-adjustment=1.3

# known address for latest (code 2854)
CONTRACT_ADDRESS=juno19u3g6cckp9whyt5h9m4f0jv3le6ry0x5xh6xzynv47f78a9sn7zq8l0xn2
CONTRACT_ADMIN_WALLET=testnet-valley
BAD_WALLET=public
BAD_WALLET_ADDRESS=$($BINARY keys show $BAD_WALLET --address)

# fund the msig so it can send
echo "Funding the multisig..."
$BINARY tx bank send $CONTRACT_ADMIN_WALLET $CONTRACT_ADDRESS 200000ujunox --fees 5000ujunox --chain-id=$CHAIN_ID --node=$RPC

# Contract already instantiated; let's try a transaction from authorized admin
# (send back to admin)
echo "Waiting, to avoid sequence mismatch error..."
sleep 10s
echo "TX 1) Admin sends the contract's funds. Should succeed."
EXECUTE_ARGS=$(jq -n '{"execute": {"msgs": [{"bank": {"send": {"to_address": "juno1hu6t6hdx4djrkdcf5hnlaunmve6f7qer9j6p9k","amount": [{"denom": "ujunox",amount: "40000"}]}}}]}}')
$BINARY tx wasm execute $CONTRACT_ADDRESS "$EXECUTE_ARGS" --from=$CONTRACT_ADMIN_WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3

# Now try to run with some other (unauthorized) wallet
echo "TX 2) Unauthorized user sends the contract's funds. Should fail."
$BINARY tx wasm execute $CONTRACT_ADDRESS "$EXECUTE_ARGS" --from=$BAD_WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3

# RM the new hot wallet (we'll add it back with higher spend limit)
echo "TX 2a... try to remove hot wallet in case a previous run terminated early."
echo "Should fail in other cases."
RM_HOT_WALLET_ARGS=$(jq -n --arg doomed $BAD_WALLET_ADDRESS '{"rm_hot_wallet": {"doomed_hot_wallet":$doomed}}')
$BINARY tx wasm execute $CONTRACT_ADDRESS "$RM_HOT_WALLET_ARGS" --from=$CONTRACT_ADMIN_WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3

# Add that other wallet as hot wallet for an hour
SECS_SINCE_EPOCH=$(date +%s)
let RESET_TIME=$SECS_SINCE_EPOCH+3600
echo "TX 3) Admin adds a new hot wallet. Should succeed."
ADD_HOT_WALLET_ARGS_V1=$(jq -n --arg newaddy $BAD_WALLET_ADDRESS '{"add_hot_wallet": {"new_hot_wallet": {"address":$newaddy, "current_period_reset":666, "period_type":"DAYS", "period_multiple":1, "spend_limits":[{"denom":"ujunox","amount":10000,"limit_remaining":10000}]}}}')
ADD_HOT_WALLET_ARGS_V2="${ADD_HOT_WALLET_ARGS_V1/666/$RESET_TIME}"
echo "Arguments: $ADD_HOT_WALLET_ARGS_V2"
$BINARY tx wasm execute $CONTRACT_ADDRESS "$ADD_HOT_WALLET_ARGS_V2" --from=$CONTRACT_ADMIN_WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3

# Now try to run the spend with the previously unauthorized hot wallet
# should FAIL since limit is too high
echo "TX 4) Hot wallet tries to spend above its limit. Should fail."
$BINARY tx wasm execute $CONTRACT_ADDRESS "$EXECUTE_ARGS" --from=$BAD_WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3

# RM the new hot wallet (we'll add it back with higher spend limit)
echo "TX 5) Admin removes the hot wallet. Should succeed."
RM_HOT_WALLET_ARGS=$(jq -n --arg doomed $BAD_WALLET_ADDRESS '{"rm_hot_wallet": {"doomed_hot_wallet":$doomed}}')
$BINARY tx wasm execute $CONTRACT_ADDRESS "$RM_HOT_WALLET_ARGS" --from=$CONTRACT_ADMIN_WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3

# error should once again read that this wallet doesn't have any hot wallet privs
echo "TX 6) Removed hot wallet tries to spend. Should fail - with error that it is not a hot wallet at all."
$BINARY tx wasm execute $CONTRACT_ADDRESS "$EXECUTE_ARGS" --from=$BAD_WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3

# add the wallet again, this time with a higher limit
# we'll do just ten blocks (60 seconds) so we can wait for limit to reset
echo "Waiting, to avoid sequence mismatch error..."
sleep 10s
echo "TX 7) Admin adds the hot wallet back, with a higher limit. Should succeed."
SECS_SINCE_EPOCH=$(date +%s)
let RESET_TIME=$SECS_SINCE_EPOCH+36
ADD_HOT_WALLET_ARGS_V1=$(jq -n --arg newaddy $BAD_WALLET_ADDRESS '{"add_hot_wallet": {"new_hot_wallet": {"address":$newaddy, "current_period_reset":666, "period_type":"DAYS", "period_multiple":1, "spend_limits":[{"denom":"ujunox","amount":45000,"limit_remaining":45000}]}}}')
ADD_HOT_WALLET_ARGS_V2="${ADD_HOT_WALLET_ARGS_V1/666/$RESET_TIME}"
$BINARY tx wasm execute $CONTRACT_ADDRESS "$ADD_HOT_WALLET_ARGS_V2$" --from=$CONTRACT_ADMIN_WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3

echo "Please complete the follow two transactions within 60 seconds so we can test reset."
# we can send the 40000
echo "TX 8) Hot wallet spends most of its limit. Should succeed."
$BINARY tx wasm execute $CONTRACT_ADDRESS "$EXECUTE_ARGS" --from=$BAD_WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3
# but then we cannot do it again since limit hasn't reset yet
echo "Waiting, to avoid sequence mismatch error..."
sleep 10s
echo "TX 9) Hot wallet tries to spend the same again. Should fail."
$BINARY tx wasm execute $CONTRACT_ADDRESS "$EXECUTE_ARGS" --from=$BAD_WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3

echo "Now waiting for the reset time to pass..."
sleep 50s
echo "Done. Transaction should succeed now."
echo "TX 10) Hot wallet tries to spend the same again after spend limit reset. Should succeed."
$BINARY tx wasm execute $CONTRACT_ADDRESS "$EXECUTE_ARGS" --from=$BAD_WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3
