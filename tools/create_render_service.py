"""Create the GIRC portal service on Render, patiently.

The API's POST bucket is far stricter than its GET bucket, so the create is
retried with the server's own reset window respected rather than burned
through. One service only: a workspace gets 750 free instance hours per
calendar month and a month is 730-744 hours, so exactly one always-on free
service fits.
"""

import json
import subprocess
import time
from pathlib import Path

ENV = Path("/home/devansh/codec_proj/.env")
key = [line.split("=", 1)[1].strip().strip('"').strip("'")
       for line in ENV.read_text().splitlines()
       if line.startswith("RENDER_API_KEY")][0]

payload = {
    "type": "web_service",
    "name": "girc-portal",
    "ownerId": "tea-daj6hdh5efls73fona0g",
    "repo": "https://github.com/devansh0703/girc",
    "branch": "master",
    # The API's autoDeploy is a string enum, not a boolean; sending a boolean
    # is rejected as "invalid JSON", which says nothing about the actual
    # problem. autoDeployTrigger is the non-deprecated spelling.
    "autoDeployTrigger": "commit",
    "envVars": [
        {"key": "PORT", "value": "8000"},
        {"key": "GIRC_MODEL", "value": "/app/models/girc-predictor.json"},
        {"key": "GIRC_BUILD_JOBS", "value": "2"},
    ],
    "serviceDetails": {
        "runtime": "docker",
        "plan": "free",
        "region": "singapore",
        "healthCheckPath": "/health",
        "envSpecificDetails": {
            "dockerfilePath": "./deploy/Dockerfile.service",
            "dockerContext": ".",
        },
    },
}

for attempt in range(10):
    r = subprocess.run(
        ["curl", "-s", "-D", "/tmp/h.txt", "-o", "/tmp/b.txt", "-w", "%{http_code}", "-X", "POST",
         "https://api.render.com/v1/services",
         "-H", f"Authorization: Bearer {key}",
         "-H", "Content-Type: application/json",
         "-H", "Accept: application/json",
         "--data-binary", json.dumps(payload)],
        capture_output=True, text=True)
    code = r.stdout.strip()
    body = open("/tmp/b.txt").read()[:600]
    print(f"{time.strftime('%H:%M:%S')} attempt {attempt}: HTTP {code} {body}", flush=True)
    if code != "429":
        break
    reset = 60
    for line in open("/tmp/h.txt"):
        if line.lower().startswith("ratelimit-reset:"):
            reset = int(line.split(":")[1].strip()) + 3
    print(f"  waiting {reset}s for the POST window", flush=True)
    time.sleep(reset)
