#!/bin/sh
set -e
mkdir ~/.near
NEAR_HOME= ~/.near
export NEAR_HOME

aws ssm get-parameter --name $CONFIG | jq -r '.Parameter.Value' > ${NEAR_HOME}/config.json
aws ssm get-parameter --name $NODE_KEYS | jq -r '.Parameter.Value' > ${NEAR_HOME}/node_key.json
aws ssm get-parameter --name $GENESIS | jq -r '.Parameter.Value' > ${NEAR_HOME}/genesis.json
if [ "$NODE_TYPE" = "validator" ]; then

    aws ssm get-parameter --name $VALIDATOR_KEYS | jq -r '.Parameter.Value' > ~/.near/validator_key.json
fi

ulimit -c unlimited

exec neard run 
