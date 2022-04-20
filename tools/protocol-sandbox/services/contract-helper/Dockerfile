FROM node:14-alpine

ENV NODE_ENV=development

WORKDIR /app
COPY package*.json .
RUN yarn install --development
COPY . .
EXPOSE 3000
RUN npx sequelize-cli migration:generate --name migration-skeleton

