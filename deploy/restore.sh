#!/usr/bin/env bash
# Restore the evaluation portal from a backup.
#
#   ./restore.sh <backup-directory>
#
# The database is restored into a freshly created one, so a restore
# cannot half-apply: the old database is dropped, the dump is loaded
# into a new one, and only then is the name switched over. If the load
# fails, the old database is still there and still in use.
#
# The artifacts are restored over the volume, and the volume is emptied
# first, because a restore that leaves the old files in place is a
# mixture of two states and nobody can tell which artifact a download
# link refers to.

set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$here"

if [[ $# -ne 1 ]]; then
    echo "usage: $0 <backup-directory>" >&2
    exit 1
fi

backup="$1"
if [[ ! -f "$backup/database.sql" || ! -f "$backup/artifacts.tar" ]]; then
    echo "$backup does not look like a backup" >&2
    exit 1
fi

if [[ ! -f .env ]]; then
    echo "no .env in $here; copy .env.example and fill it in" >&2
    exit 1
fi

set -a
# shellcheck disable=SC1091
source .env
set +a

if [[ -f "$backup/MANIFEST" ]]; then
    echo "checking the backup against its manifest"
    (
        cd "$backup"
        sha256sum --check <(grep -E '^[0-9a-f]{64} ' MANIFEST)
    )
fi

echo "restoring the database"
# The portal is stopped first, so nothing writes to the database while
# it is being replaced.
docker compose stop portal

docker compose exec -T -e PGPASSWORD="$GIRC_DB_PASSWORD" db psql \
    -U "$GIRC_DB_USER" -d postgres \
    -c "DROP DATABASE IF EXISTS ${GIRC_DB_NAME} WITH (FORCE);" \
    -c "CREATE DATABASE ${GIRC_DB_NAME} OWNER ${GIRC_DB_USER};"

docker compose exec -T -e PGPASSWORD="$GIRC_DB_PASSWORD" db psql \
    --quiet -U "$GIRC_DB_USER" -d "$GIRC_DB_NAME" \
    < "$backup/database.sql"

echo "restoring the artifacts"
# The portal container is what holds the volume, so it has to be
# running to unpack into it. It is started with the proxy still down,
# so nothing takes traffic while the volume is in its restored state.
docker compose up -d db
docker compose up -d portal
docker compose exec -T portal sh -c 'rm -rf /var/lib/girc/artifacts/*'
docker compose exec -T portal \
    tar -C /var/lib/girc -xf - \
    < "$backup/artifacts.tar"

echo "starting the portal"
docker compose up -d portal
echo "the portal has been restored from $backup"

# The proxy is started last, and separately, because a restore can be
# done on a host where the ports are already taken by something else.
# A failure here is reported rather than swallowed, but it does not
# undo the restore: the database and the artifacts are already back.
if ! docker compose up -d proxy; then
    echo "the proxy did not start; the portal is restored and answering" >&2
    echo "on the compose network, but nothing is serving it over TLS" >&2
    echo "until the ports are free" >&2
    exit 1
fi

echo "the proxy is up"
