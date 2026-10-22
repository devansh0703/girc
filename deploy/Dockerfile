# The portal, in a container.
#
# The image holds the application and nothing else. The secret is not
# in it: it is passed in as an environment variable at run time, and
# the environment file that holds it is not in the repository.

FROM python:3.12-slim AS base

ENV PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    PIP_NO_CACHE_DIR=1

# The PostgreSQL driver needs libpq, and the build tools are dropped
# again in the same layer so they do not end up in the image.
RUN apt-get update \
    && apt-get install --no-install-recommends -y libpq5 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY apps/portal/requirements.txt ./requirements.txt
RUN pip install --no-cache-dir -r requirements.txt "psycopg[binary]>=3.2"

COPY apps/portal/girc_eval ./girc_eval

# The artifacts are on a volume, not in the image.
RUN mkdir -p /var/lib/girc/artifacts \
    && useradd --system --uid 10001 girc \
    && chown -R girc:girc /app /var/lib/girc
USER girc

EXPOSE 8000

# The proxy waits for this to answer before it takes traffic.
HEALTHCHECK --interval=15s --timeout=5s --retries=5 --start-period=20s \
    CMD python -c "import urllib.request; urllib.request.urlopen('http://127.0.0.1:8000/health')"

# Two workers, because the limiter and the sessions are shared through
# Redis and the database, so more than one is safe.
CMD ["uvicorn", "girc_eval.app:app", "--host", "0.0.0.0", "--port", "8000", \
     "--workers", "2", "--proxy-headers", "--forwarded-allow-ips", "*", \
     "--no-server-header"]
