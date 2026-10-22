# Deploying the evaluation portal

The evaluation portal is the part of Product B that hands out
confidential material. It is a small application, but the things it
holds are worth protecting, so the deployment is built around four
properties:

1. **The secret is never in the image.** The signing secret, the
   database password and the Redis token are passed in as environment
   variables at run time. The file that holds them is mode 600 and is
   not in the repository.
2. **Only the proxy is exposed.** The database is not published to the
   host at all, and the portal is reached only over the compose
   network.
3. **Traffic is TLS.** Caddy obtains and renews the certificate on its
   own, and the portal is only reachable over HTTPS.
4. **The state is backed up, and the backup is checked.** The database
   and the artifacts are dumped, and the dump is verified against a
   manifest before the run is called a success.

## The stack

| Service | What it is | What it holds |
|---|---|---|
| `proxy` | Caddy | the certificate, the security headers, the access log |
| `portal` | uvicorn, two workers | the application, the artifact files |
| `db` | PostgreSQL 16 | the organisations, the agreements, the audit log |

The artifacts are on their own volume. The database is on its own
volume. Neither is published to the host.

The portal runs two workers, which is safe because the things a second
worker needs to share are shared: the sessions are signed cookies, the
database is a database, and the sign-in rate limit is in Redis.

## Bringing it up

```sh
cd deploy
cp .env.example .env
chmod 600 .env
```

Then fill in `.env`:

- `GIRC_DOMAIN` — the name the portal is served on. It has to resolve
  to this host's public address, or Caddy cannot obtain a certificate
  for it.
- `GIRC_EMAIL` — where the certificate's expiry notice goes.
- `GIRC_DB_PASSWORD` and `GIRC_SECRET_KEY` — generate the secret with
  `python3 -c 'import secrets; print(secrets.token_urlsafe(48))'`.
- `GIRC_ADMIN_EMAIL` — the first administrator's address.
- `GIRC_REDIS_URL` and `GIRC_REDIS_TOKEN` — the distributed sign-in
  rate limiter. Without them the limit is kept in the portal's own
  memory, which holds for one process and not for two.

Then:

```sh
docker compose up -d
docker compose ps
```

The portal answers on `/health`, and the proxy waits for that before
it takes traffic, so a portal that failed to start never receives a
request.

## Backups

```sh
./backup.sh
```

The backup directory is `backups/<timestamp>/` and holds:

| File | What it is |
|---|---|
| `database.sql` | `pg_dump` of the whole database, including the audit log |
| `artifacts.tar` | the artifact volume |
| `MANIFEST` | the sizes and the SHA-256 of both, and the creation time |

The script checks the dump is not empty, verifies the archive against
the manifest with `sha256sum --check`, and removes the backups older
than fourteen days.

The audit log is the record of who was given what. It is in the
database dump, so a backup that keeps the database keeps the record.

To take a backup every night, put this in a crontab:

```sh
15 2 * * * cd /path/to/deploy && ./backup.sh >> /var/log/girc-backup.log 2>&1
```

## Restoring

```sh
./restore.sh backups/20261022T021500Z
```

The restore does not half-apply. The portal is stopped, the old
database is dropped, a new one is created and the dump is loaded into
it, and the artifacts volume is emptied before the archive is unpacked
into it. If the dump fails to load, the portal has not been switched
back on, and the previous database is still the one on disk.

## The security headers

The proxy sets the headers, not the application, so they cannot be
missed by a route that forgot:

| Header | What it does |
|---|---|
| `Strict-Transport-Security` | the portal is only ever loaded over TLS |
| `Content-Security-Policy` | no third-party code, no framing, no base-uri tricks |
| `X-Frame-Options` | the portal cannot be framed by another site |
| `X-Content-Type-Options` | the browser does not guess a content type |
| `Referrer-Policy` | a download link does not leak the address it came from |
| `X-Forwarded-For` | the client address, set by the proxy, so the audit log records it |

The last one is the reason the portal's `trust_forwarded_for` setting
is on in this deployment and off by default: the header is
client-controlled unless a proxy is known to be setting it, and here
one is.

## Rotating the secret

The signing secret signs the session cookies and the download links.
Rotating it invalidates every session and every outstanding download
link, which is the correct behaviour if it has leaked and the wrong
behaviour otherwise.

```sh
cd deploy
sed -i "s|^GIRC_SECRET_KEY=.*|GIRC_SECRET_KEY=$(python3 -c 'import secrets; print(secrets.token_urlsafe(48))')|" .env
docker compose up -d --force-recreate portal
```

## What is not in the repository

- The `.env` file. It holds the signing secret, so it is mode 600 and
  ignored by git.
- The backups. They hold the database, which holds the audit log.
- The certificates. Caddy keeps them on its own volume.

`.gitignore` covers all three.
