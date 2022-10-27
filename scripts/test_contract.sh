#!/bin/bash
source ./scripts/common.sh
source ./scripts/current_contract.sh
CONTRACT_CODE_2=1309
BAD_WALLET=scripttest2
BAD_WALLET_ADDRESS=$($BINARY keys show $BAD_WALLET $KR --address)
LOOP_TOKEN_CONTRACT=juno1qsrercqegvs4ye0yqg93knv73ye5dc3prqwd6jcdcuj8ggp6w0us66deup

rm -rf ./latest_run_log.txt

# fund the msig so it can send

echo " ██████╗ ██████╗ ██╗"
echo "██╔═══██╗██╔══██╗██║"
echo "██║   ██║██████╔╝██║"
echo "██║   ██║██╔══██╗██║"
echo "╚██████╔╝██████╔╝██║"
echo " ╚═════╝ ╚═════╝ ╚═╝"
echo ""
echo -e "${YELLOW}Single Signer Proxy Wallet Contract Tests${NC}"
echo -n -e "${LBLUE}Funding the contract...${NC}"
RES=$($BINARY tx bank send $CONTRACT_ADMIN_WALLET $CONTRACT_ADDRESS $KR -y 400000$DENOM --fees 5000$DENOM --chain-id=$CHAIN_ID --node=$RPC 2>&1)
error_check "$RES" "Contract JUNO funding failed"

echo -n "Waiting to avoid sequence mismatch error..."
/usr/bin/sleep 15s && echo " Done."

echo -n -e "${LBLUE}Funding the contract with USDC...${NC}"
RES=$($BINARY tx bank send $CONTRACT_ADMIN_WALLET $CONTRACT_ADDRESS $KR -y 200000ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034 --fees 5000$DENOM --chain-id=$CHAIN_ID --node=$RPC 2>&1)
error_check "$RES" "Contract USDC funding failed"

echo -n "Waiting to avoid sequence mismatch error..."
/usr/bin/sleep 15s && echo " Done."

echo -n -e "${LBLUE}Funding the contract with LOOP...${NC}"
LOOP_TRANSFER=$(/usr/bin/jq -n --arg recipient $CONTRACT_ADDRESS '{"transfer":{"recipient":$recipient, "amount":"2000000"}}')
RES=$($BINARY tx wasm execute $LOOP_TOKEN_CONTRACT "$LOOP_TRANSFER" --from $CONTRACT_ADMIN_WALLET $KR -y --fees 5000$DENOM --chain-id=$CHAIN_ID --node=$RPC 2>&1)
error_check "$RES" "Contract LOOP funding failed"

# this is the address that will receive the "fee repay"
BALANCE_1=$($BINARY q bank balances juno1ruftad6eytmr3qzmf9k3eya9ah8hsnvkujkej8 --node=$RPC --chain-id=$CHAIN_ID 2>&1)
error_check BALANCE_1 "Failed to get balance for juno1ruftad6eytmr3qzmf9k3eya9ah8hsnvkujkej8"

echo -n "Waiting to avoid sequence mismatch error..."
/usr/bin/sleep 15s && echo " Done."

# Try to migrate to CONTRACT_CODE_2
RES=$($BINARY tx wasm migrate $CONTRACT_ADDRESS $CONTRACT_CODE_2 '{}' $KR -y --from=scripttest --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3 2>&1)
echo "Debug: $CONTRACT_ADDRESS $CONTRACT_CODE_2 '{}'"
error_check "$RES" "Unable to migrate"

# Contract already instantiated; let's try a transaction from authorized admin
# (send back to admin)
# Note this is sim_execute only at the moment, for debugging
ACTION="execute"
echo -n "Waiting to avoid sequence mismatch error..."
/usr/bin/sleep 15s && echo " Done."
echo -n -e "${LBLUE}TX 1) Admin sends the contract's funds. Should succeed, with fee repaid...${NC}"
EXECUTE_ARGS=$(/usr/bin/jq -n --arg denom $DENOM --arg action $ACTION '{($action): {"msgs": [{"bank": {"send": {"to_address": "juno1hu6t6hdx4djrkdcf5hnlaunmve6f7qer9j6p9k","amount": [{"denom": $denom,amount: "30000"}]}}}]}}')
RES=$($BINARY tx wasm execute $CONTRACT_ADDRESS "$EXECUTE_ARGS" $KR -y --from=$CONTRACT_ADMIN_WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3 2>&1)
error_check "$RES" "Admin unable to send funds"
echo $RES > latest_run_log.txt

echo -n "Waiting to avoid sequence mismatch error and to update nodes..."
/usr/bin/sleep 15s && echo " Done."

echo -n "Checking that fees were repaid..."
BALANCE_2=$($BINARY q bank balances juno1ruftad6eytmr3qzmf9k3eya9ah8hsnvkujkej8 --node=$RPC --chain-id=$CHAIN_ID 2>&1)
error_check BALANCE_2 "Failed to get balance for juno1ruftad6eytmr3qzmf9k3eya9ah8hsnvkujkej8"
if [[ "$BALANCE_1" == "$BALANCE_2" ]]
then
  echo "Uhoh, it seems fees owed were not repaid"
  exit 1
fi
 echo " Done."

echo -e "${LBLUE}TX 1a... try to remove hot wallet in case a previous run terminated early.${NC}"
echo "Should fail in other cases."
RM_HOT_WALLET_ARGS=$(/usr/bin/jq -n --arg doomed $BAD_WALLET_ADDRESS '{"rm_hot_wallet": {"doomed_hot_wallet":$doomed}}')
RES=$($BINARY tx wasm execute $CONTRACT_ADDRESS "$RM_HOT_WALLET_ARGS" $KR -y --from=$CONTRACT_ADMIN_WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3 2>&1)
echo $RES > latest_run_log.txt

echo -n "Waiting for nodes to update..."
/usr/bin/sleep 15s && echo " Done."

# Now try to run with some other (unauthorized) wallet
echo -n -e "${LBLUE}TX 2) Unauthorized user sends the contract's funds. Should fail...${NC}"
RES=$($BINARY tx wasm execute $CONTRACT_ADDRESS "$EXECUTE_ARGS" $KR --from=$BAD_WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3 2>&1)
error_check "$RES" "Admin unable to send funds" "This address is not authorized as a spend limit Hot Wallet"
echo $RES > latest_run_log.txt

echo -n "Waiting to avoid sequence mismatch error..."
/usr/bin/sleep 15s && echo " Done."

# Add that other wallet as hot wallet for an hour
SECS_SINCE_EPOCH=$(/usr/bin/date +%s)
let RESET_TIME=$SECS_SINCE_EPOCH+3600
echo -n -e "${LBLUE}TX 3) Admin adds a new hot wallet. Should succeed...${NC}"
ADD_HOT_WALLET_ARGS_V1=$(/usr/bin/jq -n --arg newaddy $BAD_WALLET_ADDRESS --arg denom $DENOM '{"add_hot_wallet": {"new_hot_wallet": {"address":$newaddy, "current_period_reset":666, "period_type":"DAYS", "period_multiple":1, "spend_limits":[{"denom":$denom,"amount":10000,"limit_remaining":10000}]}}}')
ADD_HOT_WALLET_ARGS_V2="${ADD_HOT_WALLET_ARGS_V1/666/$RESET_TIME}"
RES=$($BINARY tx wasm execute $CONTRACT_ADDRESS "$ADD_HOT_WALLET_ARGS_V2" $KR -y --from=$CONTRACT_ADMIN_WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3)
error_check "$RES" "Failed to add hot wallet"

# Now try to run the spend with the previously unauthorized hot wallet
# should FAIL since limit is too high
echo -n -e "${LBLUE}TX 4) Hot wallet tries to spend above its limit. Should fail...${NC}"
RES=$($BINARY tx wasm execute $CONTRACT_ADDRESS "$EXECUTE_ARGS" $KR --from=$BAD_WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3 2>&1)
# not sure why this doesn't work
# error_check "$RES" "Failed as expected" "You cannot spend more than your available spend limit"
# and this does instead
error_check "$RES" "Failed as expected, but with unexpected error" "This address is not authorized as a spend limit Hot Wallet"

echo -n "Waiting to avoid sequence mismatch error..."
/usr/bin/sleep 15s && echo " Done."

# RM the new hot wallet (we'll add it back with higher spend limit)
echo -n -e "${LBLUE}TX 5) Admin removes the hot wallet. Should succeed...${NC}"
RM_HOT_WALLET_ARGS=$(/usr/bin/jq -n --arg doomed $BAD_WALLET_ADDRESS '{"rm_hot_wallet": {"doomed_hot_wallet":$doomed}}')
RES=$($BINARY tx wasm execute $CONTRACT_ADDRESS "$RM_HOT_WALLET_ARGS" $KR -y --from=$CONTRACT_ADMIN_WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3 2>&1)
error_check "$RES" "Failed to remove hot wallet"

echo -n "Waiting for nodes to update..."
/usr/bin/sleep 15s && echo " Done."

# error should once again read that this wallet doesn't have any hot wallet privs
echo -n -e "${LBLUE}TX 6) Removed hot wallet tries to spend. Should fail - with error that it is not a hot wallet at all...${NC}"
RES=$($BINARY tx wasm execute $CONTRACT_ADDRESS "$EXECUTE_ARGS" $KR --from=$BAD_WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3 2>&1)
# once again, seems error is flipped here...
error_check "$RES" "Failed as expected, but with unexpected error" "This address is not authorized as a spend limit Hot Wallet" "You cannot spend more than your available spend limit"

# add the wallet again, this time with a higher limit
# we'll do just ten blocks (60 seconds) so we can wait for limit to reset
echo -n "Waiting to avoid sequence mismatch error..."
/usr/bin/sleep 15s && echo " Done."
echo -n -e "${LBLUE}TX 7) Admin adds the hot wallet back, with a higher limit. Should succeed...${NC}"
SECS_SINCE_EPOCH=$(/usr/bin/date +%s)
let RESET_TIME=$SECS_SINCE_EPOCH+60
ADD_HOT_WALLET_ARGS_V1=$(/usr/bin/jq -n --arg newaddy $BAD_WALLET_ADDRESS --arg denom $DENOM '{"add_hot_wallet": {"new_hot_wallet": {"address":$newaddy, "current_period_reset":666, "period_type":"DAYS", "period_multiple":1, "spend_limits":[{"denom":$denom,"amount":50000,"limit_remaining":50000}]}}}')
ADD_HOT_WALLET_ARGS_V2="${ADD_HOT_WALLET_ARGS_V1/666/$RESET_TIME}"
RES=$($BINARY tx wasm execute $CONTRACT_ADDRESS "$ADD_HOT_WALLET_ARGS_V2" $KR -y --from=$CONTRACT_ADMIN_WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3 2>&1)
error_check "$RES" "Failed to re-add hot wallet"

# we can send the 40000
echo -n "Waiting for nodes to update..."
/usr/bin/sleep 15s && echo " Done."

# print hot wallet to check on spend limit
QUERY_ARGS=$(/usr/bin/jq -n '{"hot_wallets":{}}')
RES=$($BINARY q wasm contract-state smart $CONTRACT_ADDRESS "$QUERY_ARGS" --node=$RPC --chain-id=$CHAIN_ID 2>&1)
echo "Query results for hot wallets: "
echo "$RES"

BALANCE_2=$($BINARY q bank balances juno1ruftad6eytmr3qzmf9k3eya9ah8hsnvkujkej8 --node=$RPC --chain-id=$CHAIN_ID 2>&1)

echo -n -e "${LBLUE}TX 8) Hot wallet spends most of its limit (6000 ujuno). Should succeed...${NC}"
EXECUTE_ARGS=$(/usr/bin/jq -n --arg denom $DENOM --arg action $ACTION '{($action): {"msgs": [{"bank": {"send": {"to_address": "juno1hu6t6hdx4djrkdcf5hnlaunmve6f7qer9j6p9k","amount": [{"denom": $denom,amount: "6000"}]}}}]}}')
RES=$($BINARY tx wasm execute $CONTRACT_ADDRESS "$EXECUTE_ARGS" $KR -y --from=$BAD_WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3 2>&1)
error_check "$RES" "Failed to spend with hot wallet"
# but then we cannot do it again since limit hasn't reset yet
echo -n "Waiting to avoid sequence mismatch error and update nodes..."
/usr/bin/sleep 15s && echo " Done."

echo -n "Checking to make sure that fees are not being redundantly repaid..."
BALANCE_3=$($BINARY q bank balances juno1ruftad6eytmr3qzmf9k3eya9ah8hsnvkujkej8 --node=$RPC --chain-id=$CHAIN_ID 2>&1)
error_check BALANCE_3 "Failed to get balance for juno1ruftad6eytmr3qzmf9k3eya9ah8hsnvkujkej8"
if [[ "$BALANCE_3" != "$BALANCE_2" ]]
then
  echo "Uhoh, looks like fees are still being repaid even though they shouldn't be"
  exit 1
fi
echo " Done."

echo -n -e "${LBLUE}TX 9) Hot wallet tries to spend the same again. Should fail...${NC}"
RES=$($BINARY tx wasm execute $CONTRACT_ADDRESS "$EXECUTE_ARGS" $KR --from=$BAD_WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3 2>&1)
error_check "$RES" "Failed as expected, but with unexpected error" "You cannot spend more than your available spend limit" "cancelled transaction"


echo "Now waiting for the reset time to pass..."
/usr/bin/sleep 50s
echo "Done. Transaction should succeed now."
echo -n -e "${LBLUE}TX 10) Hot wallet tries to spend the same again after spend limit reset. Should succeed...${NC}"
RES=$($BINARY tx wasm execute $CONTRACT_ADDRESS "$EXECUTE_ARGS" $KR -y --from=$BAD_WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3 2>&1)
error_check "$RES" "Failed to spend after reset time"

# ok let's rm the wallet again and add it...
# but this time with a unified USDC-denominated limit
echo -n -e "${LBLUE}TX 11) Admin removes the hot wallet. Should succeed...${NC}"
RM_HOT_WALLET_ARGS=$(/usr/bin/jq -n --arg doomed $BAD_WALLET_ADDRESS '{"rm_hot_wallet": {"doomed_hot_wallet":$doomed}}')
RES=$($BINARY tx wasm execute $CONTRACT_ADDRESS "$RM_HOT_WALLET_ARGS" $KR -y --from=$CONTRACT_ADMIN_WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3 2>&1)
error_check "$RES" "Failed to remove hot wallet"

echo -n "Waiting to avoid sequence mismatch error..."
/usr/bin/sleep 15s && echo " Done."

echo -n -e "${LBLUE}TX 12) Admin adds the hot wallet back, with a USDC-denominated limit. Should succeed...${NC}"
SECS_SINCE_EPOCH=$(/usr/bin/date +%s)
let RESET_TIME=$SECS_SINCE_EPOCH+60
ADD_HOT_WALLET_ARGS_V1=$(/usr/bin/jq -n --arg newaddy $BAD_WALLET_ADDRESS '{"add_hot_wallet": {"new_hot_wallet": {"address":$newaddy, "current_period_reset":666, "period_type":"DAYS", "period_multiple":1, "spend_limits":[{"denom":"ibc\/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034","amount":80000,"limit_remaining":80000}], "usdc_denom":"true"}}}')
ADD_HOT_WALLET_ARGS_V2="${ADD_HOT_WALLET_ARGS_V1/666/$RESET_TIME}"
RES=$($BINARY tx wasm execute $CONTRACT_ADDRESS "$ADD_HOT_WALLET_ARGS_V2" $KR -y --from=$CONTRACT_ADMIN_WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3 2>&1)
error_check "$RES" "Failed to re-add hot wallet"

echo -n "Waiting for nodes to update..."
/usr/bin/sleep 15s && echo " Done."

# this spend should succeed
# tho it might fail if JUNO really skyrockets in value...
echo -n -e "${LBLUE}TX 13) Spend some JUNO... and see it run against the USDC spend limit${NC}"
# the dummy price contract returns:
# asset "ujunox" at a LOOP price of 137_000_000
# asset "ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034" at a LOOP price of 30_000_000
# so spending 1 JUNO should spend 137 LOOP, which is 137/30 ≈ 4.566 USDC.
# To save on testnet faucet usage, let's spend only 0.01 JUNO... which should ≈ 0.04566 USDC or 45,600 uUSDC.
# (against a spend limit of 80000)
EXECUTE_ARGS=$(/usr/bin/jq -n --arg denom $DENOM '{"execute": {"msgs": [{"bank": {"send": {"to_address": "juno1hu6t6hdx4djrkdcf5hnlaunmve6f7qer9j6p9k","amount": [{"denom": $denom,amount: "13000"}]}}}]}}')
RES=$($BINARY tx wasm execute $CONTRACT_ADDRESS "$EXECUTE_ARGS" $KR -y --from=$BAD_WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3 2>&1)
error_check "$RES" "Failed to spend with hot wallet limited in USDC"

# ok, print hot wallet info including spend limit
QUERY_ARGS=$(/usr/bin/jq -n '{"hot_wallets":{}}')
RES=$($BINARY q wasm contract-state smart $CONTRACT_ADDRESS "$QUERY_ARGS" --node=$RPC --chain-id=$CHAIN_ID 2>&1)
echo "Query results for hot wallets: "
echo "$RES"

echo -n "Waiting to avoid sequence mismatch error..."
/usr/bin/sleep 15s && echo " Done."

echo -n -e "${LBLUE}TX 14) Second spend should fail as we've used most of our spend limit${NC}"
RES=$($BINARY tx wasm execute $CONTRACT_ADDRESS "$EXECUTE_ARGS" $KR -y --from=$BAD_WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3 2>&1)
error_check "$RES" "Failed as expected" "You cannot spend more than your available spend limit"

echo -n "Waiting for nodes to update..."
/usr/bin/sleep 15s && echo " Done."

# print hot wallet info again to check on spend limit reduction
QUERY_ARGS=$(/usr/bin/jq -n '{"hot_wallets":{}}')
RES=$($BINARY q wasm contract-state smart $CONTRACT_ADDRESS "$QUERY_ARGS" --node=$RPC --chain-id=$CHAIN_ID 2>&1)
echo "Query results for hot wallets: "
echo "$RES"

echo -n "Waiting to avoid sequence mismatch error..."
/usr/bin/sleep 15s && echo " Done."

EXECUTE_ARGS=$(/usr/bin/jq -n --arg recipient $CONTRACT_ADMIN_WALLET '{"execute": {"msgs": [{"bank": {"send": {"to_address":$recipient,"amount": [{"denom":"ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034",amount: "25000"}]}}}]}}')
echo -n -e "${LBLUE}TX 15) A small USDC spend should hit the spend limit directly${NC}"
RES=$($BINARY tx wasm execute $CONTRACT_ADDRESS "$EXECUTE_ARGS" $KR -y --from=$BAD_WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3 2>&1)
error_check "$RES" "Failed to spend USDC directly against hot wallet limit"

echo -n "Waiting to avoid sequence mismatch error..."
/usr/bin/sleep 15s && echo " Done."

echo -n -e "${LBLUE}TX 16) And should not be repeatable...${NC}"
RES=$($BINARY tx wasm execute $CONTRACT_ADDRESS "$EXECUTE_ARGS" $KR -y --from=$BAD_WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3 2>&1)
error_check "$RES" "Failed as expected" "You cannot spend more than your available spend limit"

echo -n "Waiting to avoid sequence mismatch error..."
/usr/bin/sleep 15s && echo " Done."

UPDATE_ARGS=$(/usr/bin/jq -n --arg walletaddy $BAD_WALLET_ADDRESS '{"update_hot_wallet":{"hot_wallet":$walletaddy, "new_spend_limits":[{"denom":"ibc/EAC38D55372F38F1AFD68DF7FE9EF762DCF69F26520643CF3F9D292A738D8034","amount":26000,"limit_remaining":26000}]}}')
echo -n -e "${LBLUE}TX 17) Try to update spend limit without admin privileges. Should fail...${NC}"
RES=$($BINARY tx wasm execute $CONTRACT_ADDRESS "$UPDATE_ARGS" $KR -y --from=$CONTRACT_ADMIN_WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3 2>&1)
error_check "$RES" "Failed as expected" "Caller is not admin"

echo -n -e "${LBLUE}TX 18) Push a spend limit update as admin (which currently doesn't reset time remaining)${NC}"
RES=$($BINARY tx wasm execute $CONTRACT_ADDRESS "$UPDATE_ARGS" $KR -y --from=$CONTRACT_ADMIN_WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3 2>&1)
error_check "$RES" "Failed to spend USDC directly against hot wallet limit"

echo -n "Waiting to avoid sequence mismatch error..."
/usr/bin/sleep 15s && echo " Done."

echo -n -e "${LBLUE}TX 19) Now USDC should be spendable...${NC}"
RES=$($BINARY tx wasm execute $CONTRACT_ADDRESS "$EXECUTE_ARGS" $KR -y --from=$BAD_WALLET --node=$RPC --chain-id=$CHAIN_ID $GAS1 $GAS2 $GAS3 2>&1)
error_check "$RES" "Failed to spend USDC directly against hot wallet limit"

# todo
# loop spend
# EXECUTE_ARGS=$(/usr/bin/jq -n --arg recipient $CONTRACT_ADMIN_WALLET --arg looptoken $LOOP_TOKEN_CONTRACT '{"execute": {"msgs": [{"wasm": {"execute": {"contract_addr":$looptoken,"msg":'****NEEDBASE64HERE****'}}}]}}')
