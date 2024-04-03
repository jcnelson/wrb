#!/bin/bash

set -uoe pipefail 

DB_PATH="/tmp/wrb.db"
CONTRACTS="../src/vm/contracts"

if [ -d "$DB_PATH" ]; then
   rm -r "$DB_PATH"
fi

clarity-cli initialize "$DB_PATH"

for CONTRACT_FILENAME in "wrb-ll.clar" "wrb.clar" "wrblib.clar"; do
    CONTRACT="$CONTRACTS/$CONTRACT_FILENAME"
    CONTRACT_NAME="${CONTRACT_FILENAME%%.clar}"
    FULL_CONTRACT_NAME="SP000000000000000000002Q6VF78.$CONTRACT_NAME"

    echo "Check $CONTRACT"
    clarity-cli check --contract_id "$FULL_CONTRACT_NAME" "$CONTRACT" "$DB_PATH"

    echo "Launch $CONTRACT"
    clarity-cli launch "$FULL_CONTRACT_NAME" "$CONTRACT" "$DB_PATH"
done

