#!/usr/bin/env bash
# Local development setup: builds docker image, resets DB, starts services, builds CLI.
# Usage: source ./setup_local.sh  (to keep OUBOT_SERVER in your shell)

nix build .#docker
docker load < result

docker compose down
docker rm open-uptime-bot-db 2>/dev/null || true  # Purge DB if exists
sleep 1
docker compose up --build -d db

# Wait for PostgreSQL to be ready
echo "Waiting for PostgreSQL..."
for i in $(seq 1 30); do
  if docker exec open-uptime-bot-db pg_isready -q 2>/dev/null; then
    echo "PostgreSQL is ready."
    break
  fi
  sleep 1
done

docker compose up --build -d

nix build .#cli
export OUBOT_SERVER=http://0.0.0.0:8080
echo "OUBOT_SERVER=$OUBOT_SERVER"
