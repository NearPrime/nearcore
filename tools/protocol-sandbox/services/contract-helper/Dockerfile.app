FROM phusion/baseimage:0.11

RUN curl -o /tmp/node_setup.sh "https://deb.nodesource.com/setup_12.x"

RUN bash /tmp/node_setup.sh

RUN apt-get update -qq && apt-get install -y \
    jq \
    nodejs \
    postgresql

RUN npm install -g yarn

# contract-helper
WORKDIR /usr/app
COPY . .
RUN yarn
RUN yarn migrate
# RUN node app.js >> /var/log/contract-helper.log 2>&1
COPY /scripts/run.sh /etc/service/contract-helper/run