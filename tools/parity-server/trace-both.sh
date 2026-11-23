#!/bin/bash
# usage: trace-both.sh place era ticks model
cd /var/www/swirl/tools/parity-server
export CHOCOLATE_ASSET_DIR=${ASSETDIR:-/tmp/swirl/fakeassets/assets} CHOCOLATE_DATA_DIR=/tmp/claude-1000/chocdata
node choc-trace.mjs $1 $2 $3 /tmp/swirl/par/$1.choc.json ${4:-Dummy}
ASSETS=$(dirname ${ASSETDIR:-/tmp/swirl/fakeassets/assets}) ../../target/release/examples/trace $1 $2 $3 /tmp/swirl/par/$1.swirl.json ${4:-Dummy} 2>&1 | tail -1
python3 phys-compare.py /tmp/swirl/par/$1.choc.json /tmp/swirl/par/$1.swirl.json | head -2
diff <(sed 's/^\[print\] Workspace\.[A-Za-z]*: /[print] /; s/^\[error\] Workspace.[A-Za-z]*:[0-9]*: /[error] /' /tmp/swirl/par/$1.choc.json.log) <(sed 's/^\[error\] Workspace.[A-Za-z]*:[0-9]*: /[error] /' /tmp/swirl/par/$1.swirl.json.log) && echo SAME_LOG
