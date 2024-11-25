#!/bin/bash

set -uoe pipefail 

DB_PATH="/tmp/wrb.db"
CONTRACTS="../src/vm/contracts"

if [ -d "$DB_PATH" ]; then
   rm -r "$DB_PATH"
fi

clarity-cli initialize "$DB_PATH"

function do_checks() {
   local CONTRACT="$1"
   local CONTRACT_BASENAME="$(basename "$CONTRACT")"
   local CONTRACT_NAME="${CONTRACT_BASENAME%%.linked}"
   CONTRACT_NAME="${CONTRACT_NAME%%.clar}"
   local FULL_CONTRACT_NAME="SP000000000000000000002Q6VF78.$CONTRACT_NAME"

   echo "Check $CONTRACT"
   clarity-cli check --contract_id "$FULL_CONTRACT_NAME" "$CONTRACT" "$DB_PATH"

   echo "Launch $CONTRACT"
   clarity-cli launch "$FULL_CONTRACT_NAME" "$CONTRACT" "$DB_PATH"
}

for CONTRACT_FILENAME in "wrb-ll.clar" "wrb.clar" "wrblib.clar"; do
    do_checks "$CONTRACTS/$CONTRACT_FILENAME"
done

for arg in $@; do
    # link in wrblib
    cat "$CONTRACTS/wrblib.clar" > "$arg.linked"
    echo ";; =========== END OF WRBLIB ================" >> "$arg.linked"
    cat "$arg" >> "$arg.linked"
    do_checks "$arg.linked"
done

