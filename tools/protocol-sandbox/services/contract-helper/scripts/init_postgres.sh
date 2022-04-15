#!/bin/bash
set -e

su - postgres -c "
    /usr/lib/postgresql/10/bin/initdb -D /srv/near/postgresql/10/main &&
    /etc/init.d/postgresql start &&
    psql -c \"CREATE USER helper password 'helper'\" &&
    createdb -O helper accounts_development"
