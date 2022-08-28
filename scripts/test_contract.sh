#!/bin/bash
source ./scripts/common.sh
source ./scripts/current_contract.sh
BAD_WALLET=badtester
BAD_WALLET_ADDRESS=$($BINARY keys show $BAD_WALLET $KR --address)

rm -rf ./latest_run_log.txt

# fund the msig so it can send
echo -n "Funding the multisig..."
RES=$($BINARY tx bank send $CONTRACT_ADMIN_WALLET $CONTRACT_ADDRESS $KR -y 200000$DENOM --fees 5000$DENOM --chain-id=$CHAIN_ID --node=$RPC 2>&1)
error_check "$RES" "Multisig funding failed"
echo $RES > latest_run_log.txt

# Contract already instantiated; let's try a transaction from authorized admin
# (send back to admin)
echo -n "Waiting, to avoid sequence mismatch error..."
sleep 10s && echo " Done."
echo -n "TX 1) Admin sends the contract's funds. Should succeed..."
EXECUTE_ARGS=$(jq -n '{"execute": {"msgs": [{"bank": {"send": {"to_address": "juno1hu6t6hdx4djrkdcf5hnlaunmve6f7qer9j6p9k","amount": [{"denom": "$DENOM",amount: "40000"}]}}}]}}')
RES=$($BINARY tx wasm execute $CONTRACT_ADDRESS "$EXECUTE_ARGS" $KR -y --from=$CONTRACT_ADMIN_WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3 2>&1)
error_check "$RES" "Admin unable to send funds"
echo $RES > latest_run_log.txt

# Now try to run with some other (unauthorized) wallet
echo -n "TX 2) Unauthorized user sends the contract's funds. Should fail..."
RES=$($BINARY tx wasm execute $CONTRACT_ADDRESS "$EXECUTE_ARGS" $KR --from=$BAD_WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3 2>&1)
error_check "$RES" "Admin unable to send funds" "This address is not authorized as a spend limit Hot Wallet"
echo $RES > latest_run_log.txt

# RM the new hot wallet (we'll add it back with higher spend limit)
echo "TX 2a... try to remove hot wallet in case a previous run terminated early."
echo "Should fail in other cases."
RM_HOT_WALLET_ARGS=$(jq -n --arg doomed $BAD_WALLET_ADDRESS '{"rm_hot_wallet": {"doomed_hot_wallet":$doomed}}')
RES=$($BINARY tx wasm execute $CONTRACT_ADDRESS "$RM_HOT_WALLET_ARGS" $KR -y --from=$CONTRACT_ADMIN_WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3 2>&1)
echo $RES > latest_run_log.txt

echo -n "Waiting, to avoid sequence mismatch error..."
sleep 10s && echo " Done."

# Add that other wallet as hot wallet for an hour
SECS_SINCE_EPOCH=$(date +%s)
let RESET_TIME=$SECS_SINCE_EPOCH+3600
echo -n "TX 3) Admin adds a new hot wallet. Should succeed..."
ADD_HOT_WALLET_ARGS_V1=$(jq -n --arg newaddy $BAD_WALLET_ADDRESS '{"add_hot_wallet": {"new_hot_wallet": {"address":$newaddy, "current_period_reset":666, "period_type":"DAYS", "period_multiple":1, "spend_limits":[{"denom":"$DENOM","amount":10000,"limit_remaining":10000}]}}}')
ADD_HOT_WALLET_ARGS_V2="${ADD_HOT_WALLET_ARGS_V1/666/$RESET_TIME}"
RES=$($BINARY tx wasm execute $CONTRACT_ADDRESS "$ADD_HOT_WALLET_ARGS_V2" $KR -y --from=$CONTRACT_ADMIN_WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3)
error_check "$RES" "Failed to add hot wallet"

# Now try to run the spend with the previously unauthorized hot wallet
# should FAIL since limit is too high
echo -n "TX 4) Hot wallet tries to spend above its limit. Should fail..."
RES=$($BINARY tx wasm execute $CONTRACT_ADDRESS "$EXECUTE_ARGS" $KR --from=$BAD_WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3 2>&1)
error_check "$RES" "Failed as expected" "You cannot spend more than your available spend limit"

# RM the new hot wallet (we'll add it back with higher spend limit)
echo -n "TX 5) Admin removes the hot wallet. Should succeed..."
RM_HOT_WALLET_ARGS=$(jq -n --arg doomed $BAD_WALLET_ADDRESS '{"rm_hot_wallet": {"doomed_hot_wallet":$doomed}}')
RES=$($BINARY tx wasm execute $CONTRACT_ADDRESS "$RM_HOT_WALLET_ARGS" $KR -y --from=$CONTRACT_ADMIN_WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3 2>&1)
error_check "$RES" "Failed to remove hot wallet"

# error should once again read that this wallet doesn't have any hot wallet privs
echo -n "TX 6) Removed hot wallet tries to spend. Should fail - with error that it is not a hot wallet at all..."
RES=$($BINARY tx wasm execute $CONTRACT_ADDRESS "$EXECUTE_ARGS" $KR --from=$BAD_WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3 2>&1)
error_check "$RES" "Failed as expected" "This address is not authorized as a spend limit Hot Wallet"

# add the wallet again, this time with a higher limit
# we'll do just ten blocks (60 seconds) so we can wait for limit to reset
echo -n "Waiting, to avoid sequence mismatch error..."
sleep 10s && echo " Done."
echo -n "TX 7) Admin adds the hot wallet back, with a higher limit. Should succeed..."
SECS_SINCE_EPOCH=$(date +%s)
let RESET_TIME=$SECS_SINCE_EPOCH+60
ADD_HOT_WALLET_ARGS_V1=$(jq -n --arg newaddy $BAD_WALLET_ADDRESS '{"add_hot_wallet": {"new_hot_wallet": {"address":$newaddy, "current_period_reset":666, "period_type":"DAYS", "period_multiple":1, "spend_limits":[{"denom":"$DENOM","amount":45000,"limit_remaining":45000}]}}}')
ADD_HOT_WALLET_ARGS_V2="${ADD_HOT_WALLET_ARGS_V1/666/$RESET_TIME}"
RES=$($BINARY tx wasm execute $CONTRACT_ADDRESS "$ADD_HOT_WALLET_ARGS_V2$" $KR -y --from=$CONTRACT_ADMIN_WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3 2>&1)
error_check "$RES" "Failed to re-add hot wallet"

echo "Please complete the follow two transactions within 60 seconds so we can test reset."
# we can send the 40000
echo -n "TX 8) Hot wallet spends most of its limit. Should succeed..."
RES=$($BINARY tx wasm execute $CONTRACT_ADDRESS "$EXECUTE_ARGS" $KR -y --from=$BAD_WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3 2>&1)
error_check "$RES" "Failed to spend with hot wallet"
# but then we cannot do it again since limit hasn't reset yet
echo -n "Waiting, to avoid sequence mismatch error..."
sleep 10s && echo " Done."
echo -n "TX 9) Hot wallet tries to spend the same again. Should fail..."
RES=$($BINARY tx wasm execute $CONTRACT_ADDRESS "$EXECUTE_ARGS" $KR --from=$BAD_WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3 2>&1)
error_check "$RES" "Failed as expected" "You cannot spend more than your available spend limit"

echo "Now waiting for the reset time to pass..."
sleep 50s
echo "Done. Transaction should succeed now."
echo -n "TX 10) Hot wallet tries to spend the same again after spend limit reset. Should succeed..."
RES=$($BINARY tx wasm execute $CONTRACT_ADDRESS "$EXECUTE_ARGS" $KR -y --from=$BAD_WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3 2>&1)
error_check "$RES" "Failed to spend after reset time"
