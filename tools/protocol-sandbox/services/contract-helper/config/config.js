module.exports = {
    development: {
        username: 'contract_helper',
        password: 'contract_helper',
        database: 'contract_helper',
        host: '172.20.0.7',
        dialect: 'postgres',
    },
    test: {
        username: 'helper',
        password: 'helper',
        database: 'accounts_test',
        host: '127.0.0.1',
        dialect: 'postgres',
        logging: false
    },
    production: {
        username: process.env.HELPER_DB_USERNAME || 'helper',
        password: process.env.HELPER_DB_PASSWORD || 'helper',
        database: process.env.HELPER_DB_NAME || 'accounts_production',
        host:  process.env.HELPER_DB_HOST || '127.0.0.1',
        dialect: 'postgres',
    },
};
