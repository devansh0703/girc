# Deploying the portal and the hosted analysis

The portal is the part of GIRC that hands out confidential material, and the
hosted analysis is the part that makes the measurement runnable by somebody
who did not build a toolchain. They are two doors into one product, and the
deployment is built around four properties:

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

## The address the portal is served on

The evaluation site's pages carry a `{{PORTAL_URL}}` token, and the
address is substituted when the site is assembled. It is a repository
setting, not a secret, and it is set in `.github/workflows/pages.yml`:

```yaml
env:
  GIRC_PORTAL_URL: https://girc-portal.onrender.com
```

The portal itself is a separate deployment with its own domain, its own
certificate and its own secrets, and it is not served from the
project's pages. The pages under `/evaluate/` are the public face of
it: what the evaluation is, how it runs, and what is out of scope.

## The hosted analysis on Render

The portal runs on Render as one free web service, because a workspace
gets 750 free instance hours a month and a month is 730 to 744 hours, so
exactly one always-on free service fits. The manifest is in the
repository, so what is deployed can be compared with what is committed:

```yaml
# render.yaml
services:
  - type: web
    name: girc-portal
    runtime: python          # not Docker: free services cannot deploy Docker
    plan: free
    region: oregon
    healthCheckPath: /health
    buildCommand: ...        # downloads the codec CI built, installs the service
    startCommand: PYTHONPATH=apps/portal python3 -m girc_eval.service
```

The runtime is native python and the codec arrives as a download, and
both facts are forced by the free tier: a Docker deploy on a free
service fails within seconds, and the free build plan has too little
memory to compile the Rust codec (the first attempt died two minutes
into `cargo`). So the build command fetches the `codec-linux` release
asset — the binary the Rust job in CI publishes from the same commit
that deploys — and runs the service against it. One implementation of
the measurement: the binary over HTTP is the binary on the command line,
and a push moves both together.

The service can also be created through the API
(`tools/create_render_service.py` does it), and the request shape is
not what the error messages suggest:

- `repo`, `branch` and `envVars` belong at the **top level**; only the
  runtime knobs belong inside `serviceDetails`, with `buildCommand` and
  `startCommand` nested one level deeper under
  `serviceDetails.envSpecificDetails`.
- `autoDeploy` is a **string enum** (`"yes"`/`"no"`), not a boolean; use
  `autoDeployTrigger: "commit"` instead. Sending a boolean gets the
  useless reply `{"message":"invalid JSON"}`, which is what the API
  returns for any schema violation, valid JSON or not.
- The POST bucket on `/v1/services` is far stricter than the GET
  bucket — a handful of requests per window, and the window is minutes
  long. The script reads `ratelimit-reset` and waits rather than
  burning attempts.
- PATCHing a live service from one runtime to another is accepted and
  then fails every deploy. Create the service with the shape you mean.

A free account must have a payment card on file before the API will
create any service, even a free one. The dashboard and the API share
this rule.

A free instance sleeps after fifteen minutes without a request, so an
UptimeRobot monitor pings `/health` every five minutes. The monitor is
created with the UptimeRobot API and the key is never in the repository:

```sh
curl -X POST https://api.uptimerobot.com/v2/newMonitor \
  -d "api_key=$UPTIMEROBOT_API_KEY&format=json" \
  -d "friendly_name=GIRC portal (Render free instance)" \
  -d "url=https://girc-portal.onrender.com/health&type=1" \
  -d "interval=300"
```

Render's cron jobs are a paid instance type, so the free workspace
schedules nothing and the scheduled canary lives in GitHub Actions
instead: `.github/workflows/service.yml` calls the service over HTTP
once a week, which keeps the one-instance budget intact and uses a
scheduler that already exists in the repository.

The portal's address is
**[girc-portal.onrender.com](https://girc-portal.onrender.com/)**. The
analysis endpoint is `/analyze`, which takes a sequence and returns the
figures; `/register` is the evaluation portal.

## What is not in the repository

- The `.env` file. It holds the signing secret, so it is mode 600 and
  ignored by git.
- The backups. They hold the database, which holds the audit log.
- The certificates. Caddy keeps them on its own volume.

`.gitignore` covers all three.
