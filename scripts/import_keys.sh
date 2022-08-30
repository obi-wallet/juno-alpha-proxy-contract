#!/usr/bin/expect -f

set timeout -1
spawn bash ./scripts/import_key.sh scripttest scripttest2

expect "> Enter your bip39 mnemonic\r"

send -- "$env(BASHTESTER)\r"

expect "> Enter your bip39 mnemonic\r"

send -- "$env(BADTESTER)\r"

expect eof