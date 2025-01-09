#!/usr/bin/env bash
set -x
set -eo pipefail

if ! [ -x "$(command -v psql)" ]; then
  echo >&2 "Error: psql is not installed."
  exit 1
fi

if ! [ -x "$(command -v sea)" ]; then
  echo >&2 "Error: sea-orm-cli is not installed."
  echo >&2 "Use:"
  echo >&2 "    cargo install sea-orm-cli"
  echo >&2 "to install it."
  exit 1
fi

# Check if a custom parameter has been set, otherwise using default values
DB_PORT="${POSTGRES_PORT:=5432}"
SUPERUSER="${SUPERUSER:=postgres}"
SUPERUSER_PWD="${SUPERUSER_PWD:=password}"
APP_USER="${APP_USER:=app}"
APP_USER_PWD="${APP_USER_PWD:=secret}"
APP_DB_NAME="${APP_DB_NAME:=newsletter}"

if [[ -z "${SKIP_DOCKER}" ]]
then
  # Launch postgres using Docker
  CONTAINER_NAME="newsletter"

  # Stop and delete last used container
  if [[ $(! docker container ls -a | grep "${CONTAINER_NAME}") ]]
  then
    docker container stop ${CONTAINER_NAME}
    docker container rm ${CONTAINER_NAME}
  fi

  docker run \
    --env POSTGRES_USER="${SUPERUSER}" \
    --env POSTGRES_PASSWORD="${SUPERUSER_PWD}" \
    --health-cmd="pg_isready -U ${SUPERUSER} || exit 1" \
    --health-interval=1s \
    --health-timeout=10s \
    --health-retries=5 \
    --publish "${DB_PORT}":5432 \
    --detach \
    --name "${CONTAINER_NAME}" \
    postgres -N 1000  # Increase maximum number of connections for testing purpose

  until [ \
    "$(docker inspect -f "{{.State.Health.Status}}" "${CONTAINER_NAME}")" == "healthy" \
  ]; do
    >&2 echo "Postgres is still unavailable - waiting..."
    sleep 1
  done

  # Create the app user
  CREATE_QUERY="CREATE USER ${APP_USER} WITH PASSWORD '${APP_USER_PWD}';"
  docker exec -it "${CONTAINER_NAME}" psql -U "${SUPERUSER}" -c "${CREATE_QUERY}"

  # Grant create db privileges to the app user
  GRANT_QUERY="ALTER USER ${APP_USER} CREATEDB;"
  docker exec -it "${CONTAINER_NAME}" psql -U "${SUPERUSER}" -c "${GRANT_QUERY}"
  
  # Create the application database
  CREATE_DATABASE_QUERY="CREATE DATABASE ${APP_DB_NAME};"
  docker exec -it "${CONTAINER_NAME}" psql -U "${APP_USER}" -d postgres -c "${CREATE_DATABASE_QUERY}"
fi

>&2 echo "Postgres is up and running on port ${DB_PORT} - running migrations..."

DATABASE_URL=postgres://${APP_USER}:${APP_USER_PWD}@localhost:${DB_PORT}/${APP_DB_NAME}
export DATABASE_URL

sea-orm-cli migrate

>&2 echo "Postgres has been migrated, ready to go."

