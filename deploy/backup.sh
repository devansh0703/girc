#!/usr/bin/env bash
# Back up the evaluation portal.
#
# Two things hold state worth keeping: the database, which has the
# organisations, the agreements and the audit log, and the artifacts
# volume, which has the gated material. The database is dumped with
# pg_dump, because a copy of the database files taken while PostgreSQL
# is running is not a backup. The artifacts are copied, because they
# are files.
#
# The audit log is the record of who was given what, so a backup that
# skips it is not a backup of the thing that matters.
#
# The backups are kept for fourteen days and the oldest are removed
# after each run, so the disk cannot fill.
#
#   ./backup.sh
#
# To restore:
#
#   ./restore.sh <backup-directory>

set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$here"

if [[ ! -f .env ]]; then
    echo "no .env in $here; copy .env.example and fill it in" >&2
    exit 1
fi

# The environment file holds the values, and the passwords in it may
# hold characters the shell would otherwise interpret.
set -a
# shellcheck disable=SC1091
source .env
set +a

if [[ -z "${GIRC_DB_PASSWORD:-}" || -z "${GIRC_SECRET_KEY:-}" ]]; then
    echo "the database password and the secret key must be set in .env" >&2
    exit 1
fi

stamp="$(date -u +%Y%m%dT%H%M%SZ)"
destination="${BACKUP_DIR:-./backups}/$stamp"
mkdir -p "$destination"

echo "backing up the database to $destination"
docker compose exec -T \
    -e PGPASSWORD="$GIRC_DB_PASSWORD" \
    db pg_dump --no-owner --no-acl --clean --if-exists \
    -U "$GIRC_DB_USER" -d "$GIRC_DB_NAME" \
    > "$destination/database.sql"

# A dump that is empty is not a backup, so it is checked.
if [[ ! -s "$destination/database.sql" ]]; then
    echo "the database dump is empty; the backup has failed" >&2
    exit 1
fi

echo "backing up the artifacts"
docker compose exec -T portal \
    tar -C /var/lib/girc -cf - artifacts \
    > "$destination/artifacts.tar"

if [[ ! -s "$destination/artifacts.tar" ]]; then
    echo "the artifact archive is empty; the backup has failed" >&2
    exit 1
fi

# The manifest lets a restore check that the backup is the one it
# thinks it is.
{
    echo "created: $stamp"
    echo "database: $GIRC_DB_NAME"
    echo "artifact archive bytes: $(stat -c %s "$destination/artifacts.tar")"
    echo "database dump bytes: $(stat -c %s "$destination/database.sql")"
    # The names are relative to the backup directory, so the manifest
    # can be checked from inside it.
    (cd "$destination" && sha256sum database.sql artifacts.tar)
} > "$destination/MANIFEST"

echo "verifying the backup"
(
    cd "$destination"
    sha256sum --check <(grep -E '^[0-9a-f]{64} ' MANIFEST)
)

echo "removing the backups older than fourteen days"
find "${BACKUP_DIR:-./backups}" -mindepth 1 -maxdepth 1 -type d -mtime +14 \
    -exec rm -rf {} + 2>/dev/null || true

echo "the backup is at $destination"
