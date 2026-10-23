---
name: girc-product
description: Use when working in the GIRC repository — regenerating figures or the report, editing the product site, running the verification chain, or operating the Render/UptimeRobot deployment.
---

# GIRC — the product repository's workflows

One product, four parts: the codec (`crates/`), the training pipeline
(`python/girc-train`), the benchmark (`crates/girc-bench`), and the portal
plus hosted analysis (`apps/portal`, `deploy/`). The site
(`web/girc-site`) and the report (`research/REPORT.md`) are **generated
artifacts**; never hand-edit their figures or numbers.

## The invariants a reviewer should enforce

1. Every figure is generated from a committed artifact by
   `tools/generate_figures.py`. A page carrying a figure the generator
   did not produce is a bug.
2. Every number in README, site, report or docs must appear in the
   benchmark report JSON; `tools/verify_benchmark_numbers.py` checks
   this and catches drift between documents.
3. Light is the site's default theme; dark is opt-in via the inline
   bootstrap script (never `prefers-color-scheme`).
4. No placeholder addresses anywhere (`example.com`, `.invalid`,
   lorem). The real contact is devanshraulo@gmail.com; the service is
   https://girc-portal.onrender.com (substituted via the
   `{{PORTAL_URL}}` token).
5. The site is self-contained: fonts (Plex + Phosphor) are inlined in
   `assets/site.css` as data URLs. After adding or removing an icon
   class in any page, re-run `tools/fetch_fonts.py`.

## The verification chain (run before any commit touching these)

```sh
cargo test --workspace                                  # 87 tests
PYTHONPATH=apps/portal pytest apps/portal/tests -q      # 42 tests
.venv/bin/python tools/generate_figures.py              # 18 figures
.venv/bin/python tools/generate_report.py               # research/REPORT.md
.venv/bin/python tools/verify_benchmark_numbers.py research/benchmark/report-v9-joint.json
.venv/bin/python tools/verify_claim_charts.py           # prior-art quotes
.venv/bin/python tools/assemble_site.py                 # build/site + checks
.venv/bin/python tools/check_design_contract.py
.venv/bin/python tools/check_site_links.py
```

CI runs the same chain (`.github/workflows/ci.yml`), so a red local run
will be red there.

## Editing pages

- Pages live in `web/girc-site/` (`evaluate/` is a section, not a site).
  Insert figures with `{{FIGURE:fig-kebab-name}}`; the assembler
  substitutes the generated SVG and fails if the name does not exist.
- Figure filenames use hyphens (`fig-rd-curves.svg`); the token must
  too.
- Every page needs at least two figures; the assembler enforces it.
- The design contract (no gradients, no uppercase mono kickers, no
  pill badges, no emoji icons, Phosphor only, amber accent) is enforced
  by `tools/check_design_contract.py` across the stylesheet and pages.

## The hosted analysis (Render)

- One free service, because 750 free instance-hours/month is one
  always-on instance. `render.yaml` is the manifest.
- The container (`deploy/Dockerfile.service`) builds the Rust CLI and
  the Python service shells out to it — one implementation of the
  measurement, not two.
- Create through the API with `tools/create_render_service.py`.
  Gotchas baked into that script: `repo`/`branch`/`envVars` are
  top-level; docker paths go under `serviceDetails.envSpecificDetails`;
  `autoDeploy` is a string enum (use `autoDeployTrigger: "commit"`);
  the POST bucket is tiny, so respect `ratelimit-reset`.
- UptimeRobot monitor `804394858` pings `/health` every 300 s so the
  free instance never sleeps. The weekly canary is
  `.github/workflows/service.yml` (GitHub Actions, because Render cron
  is paid).
- After a push to master, watch the deploy via
  `GET /v1/services` and `GET /v1/services/{id}/deploys`, then verify
  `/health` publishes the limits and `/analyze` on a real sequence.

## Pitfalls found the hard way

- The theme script once fell back to `prefers-color-scheme`; the
  browser then renders dark by default. Light must win when nothing is
  stored.
- `service.analyse` must accept both `W192 H192` and `W192H192` Y4M
  headers; the real Xiph sequences use the spaced form.
- The claim-chart verifier expects the counts sentence to match its
  chart exactly; keep that sentence in one unwrapped line.
- `verify_benchmark_numbers.py` fails when a document quotes a number
  absent from the report — that is its job, not a flake.
