# syntax=docker/dockerfile-upstream:experimental

FROM ubuntu:18.04 as build

RUN apt-get update -qq && apt-get install -y \
    git \
    cmake \
    g++ \
    pkg-config \
    libssl-dev \
    curl \
    llvm \
    clang \
    && rm -rf /var/lib/apt/lists/*

COPY ./rust-toolchain.toml /tmp/rust-toolchain.toml

ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH

RUN curl https://sh.rustup.rs -sSf | \
    sh -s -- -y --no-modify-path --default-toolchain none

VOLUME [ /near ]
WORKDIR /near
COPY . .

ENV PORTABLE=ON
RUN make release && cp /near/target/release/neard /tmp/


# Actual image
FROM ubuntu:18.04

EXPOSE 3030 24567

RUN apt-get update -qq && apt-get install -y \
    libssl-dev ca-certificates wget\
    && rm -rf /var/lib/apt/lists/*

COPY scripts/run_docker.sh /usr/local/bin/run.sh
COPY --from=build /tmp/neard /usr/local/bin/

RUN     neard --home ~/.near init --chain-id mainnet --download-genesis --download-config

RUN     rm ~/.near/config.json ~/.near/genesis.json  
# wget https://s3-us-west-1.amazonaws.com/build.nearprotocol.com/nearcore-deploy/mainnet/config.json -P ~/.near/

ARG config_json=""
ARG genesis_json=""

ENTRYPOINT [ "neard", "--home", "/root/.near", "run" ]


