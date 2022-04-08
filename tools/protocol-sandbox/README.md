### Setup

1. To build the docker image this command will
    ```bash
    cd ../../../ docker build -f tools/protocol-sandbox/docker/Dockerfile -t nearcore_local_test:latest .
    ```
2. Build the contract helper image
    ``` bash 
    docker build -f tools/protocol-sandbox/docker/contract-helper/Dockerfile -t nearcore_local_contract_helper:latest .
    ```
3. Run the docker compose file from within the `tools/protocol-sandbox/docker` directory
    ```bash
    docker-compose up -d 
    ```
4. To stop the services run
    ```bash
    docker-compose down
    ```
   
- The data for each node will persist within the data directory for each config directory
  - To remove the directories with the containers off running
    ```bash
    sudo rm -rf ../configs/*/data
    ```
