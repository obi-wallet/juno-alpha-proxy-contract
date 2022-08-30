source ./scripts/common.sh

$BINARY keys add $1 --recover --keyring-backend=test
$BINARY keys add $2 --recover --keyring-backend=test