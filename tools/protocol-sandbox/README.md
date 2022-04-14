### Setup

~~1. To build the docker image this command will.
    ```bash
    cd ../../../ && docker build -f tools/protocol-sandbox/docker/Dockerfile -t nearcore_local_test:latest .
    ```~~
~~2. Build the contract helper image from within the `tools/protocol-sandbox/docker/contract-helper` directory.
    ``` bash 
    docker build -f Dockerfile -t nearcore_local_contract_helper:latest .
    ```~~
3. Run the docker compose file from within the `tools/protocol-sandbox/docker` directory. You can pass the `--build` flag to the following command to force build the services with a build tag.
    ```bash
    docker-compose up -d 
    ```
4. To stop the services run
    ```bash
    docker-compose down
    ```
   
- The data for each node & DB will persist within the data directory for each directory
  - To start from a clean state run:
    ```bash
    sudo rm -rf ./configs/*/data docker/postgres-db/data
    ```
