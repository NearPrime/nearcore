#!/bin/sh
set -e

mkdir ~/.near
aws ssm get-parameter --name $CONFIG | jq -r '.Parameter.Value' > ~/.near/config.json
aws ssm get-parameter --name $NODE_KEYS | jq -r '.Parameter.Value' > ~/.near/node_key.json
aws ssm get-parameter --name $GENESIS | jq -r '.Parameter.Value' > ~/.near/genesis.json
if [ "$NODE_TYPE" = "validator" ]; then

    aws ssm get-parameter --name $VALIDATOR_KEYS | jq -r '.Parameter.Value' > ~/.near/validator_key.json
fi

ulimit -c unlimited

exec neard --home "/root/.near" run 
