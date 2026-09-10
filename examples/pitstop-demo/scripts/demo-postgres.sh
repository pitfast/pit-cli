#!/usr/bin/env bash
set -euo pipefail

container="${PITFAST_DEMO_POSTGRES_CONTAINER:-pitfast-postgres-demo}"
image="${PITFAST_DEMO_POSTGRES_IMAGE:-postgres:16}"
superuser="${PITFAST_DEMO_POSTGRES_SUPERUSER:-pitfast}"
database="${PITFAST_DEMO_POSTGRES_DATABASE:-pitfast_demo}"
gateway_role="${PITFAST_DEMO_POSTGRES_GATEWAY_ROLE:-pit}"

if [[ ! "$container" =~ ^[A-Za-z0-9_.-]+$ ]]; then echo "FAIL: invalid PostgreSQL container name" >&2; exit 2; fi
if [[ ! "$superuser" =~ ^[A-Za-z0-9_]+$ ]]; then echo "FAIL: invalid PostgreSQL superuser name" >&2; exit 2; fi
if [[ ! "$database" =~ ^[A-Za-z0-9_]+$ ]]; then echo "FAIL: invalid PostgreSQL database name" >&2; exit 2; fi
if [[ ! "$gateway_role" =~ ^[A-Za-z0-9_]+$ ]]; then echo "FAIL: invalid PostgreSQL gateway role name" >&2; exit 2; fi

command -v docker >/dev/null 2>&1 || { echo "FAIL: docker is required" >&2; exit 1; }
if ! docker info >/dev/null 2>&1; then
  echo "FAIL: Docker daemon is not accessible; refresh the docker group with 'newgrp docker' or start a new login session" >&2
  exit 1
fi

if ! docker inspect "$container" >/dev/null 2>&1; then
  echo "Creating demo-owned PostgreSQL container: $container"
  docker run --name "$container" \
    -e POSTGRES_USER="$superuser" \
    -e POSTGRES_PASSWORD="${PITFAST_DEMO_POSTGRES_PASSWORD:-pitfast-demo-local}" \
    -e POSTGRES_DB="$database" \
    -p 127.0.0.1:5432:5432 \
    -v "$container:/var/lib/postgresql/data" \
    -d "$image" >/dev/null
elif [ "$(docker inspect -f '{{.State.Running}}' "$container")" != "true" ]; then
  echo "Starting existing demo-owned PostgreSQL container: $container"
  docker start "$container" >/dev/null
fi

for _ in $(seq 1 60); do
  if docker exec "$container" pg_isready -U "$superuser" -d "$database" >/dev/null 2>&1; then
    break
  fi
  sleep 1
done
docker exec "$container" pg_isready -U "$superuser" -d "$database" >/dev/null \
  || { echo "FAIL: PostgreSQL did not become ready" >&2; exit 1; }

if ! docker exec "$container" psql -Atqc "SELECT 1 FROM pg_database WHERE datname = '$database'" -U "$superuser" -d postgres | grep -qx 1; then
  docker exec "$container" createdb -U "$superuser" "$database"
fi

docker exec "$container" psql -v ON_ERROR_STOP=1 -U "$superuser" -d postgres -c \
  "DO \$\$ BEGIN CREATE ROLE $gateway_role LOGIN; EXCEPTION WHEN duplicate_object THEN NULL; END \$\$;" >/dev/null
docker exec "$container" psql -v ON_ERROR_STOP=1 -U "$superuser" -d "$database" -c \
  "GRANT CONNECT ON DATABASE $database TO $gateway_role; GRANT USAGE, CREATE ON SCHEMA public TO $gateway_role;" >/dev/null

bridge_gateway="$(docker inspect -f '{{range .NetworkSettings.Networks}}{{.Gateway}}{{end}}' "$container")"
[ -n "$bridge_gateway" ] || { echo "FAIL: could not determine Docker bridge gateway" >&2; exit 1; }
hba_entry="host all $gateway_role $bridge_gateway/32 trust"
docker exec -u postgres "$container" sh -c \
  'grep -Fqx "$1" /var/lib/postgresql/data/pg_hba.conf || sed -i "1i$1" /var/lib/postgresql/data/pg_hba.conf' \
  sh "$hba_entry"
docker exec "$container" psql -U "$superuser" -d postgres -c 'SELECT pg_reload_conf()' >/dev/null

echo "PostgreSQL demo ready"
echo "Container: $container ($image)"
echo "Database: $database"
echo "Gateway role: $gateway_role"
echo "Host bridge trust source: $bridge_gateway/32 (local demo only)"
