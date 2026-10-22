# TODOS — GIRC (Geometry-Inference Codec)

Status legend: `[ ]` todo · `[~]` in progress · `[x]` done · `[-]` cut

## Product A — GIRC toolkit (crates/girc-*, python/girc-train)
- [x] girc-core: VVC integer transforms, bit-exact against VVenC
- [x] girc-core: the four per-axis multiple transform selection modes,
      bit-exact against VVenC, over all sixteen mode and size combinations
- [x] girc-core: the mts_idx signalling, matching the standard's syntax
- [x] girc-core: entropy coding and the bitstream container
- [x] girc-core: intra prediction and rate-distortion search
- [x] girc-core: the geometry predictor, with the neighbour-transform context
- [x] girc-core: the per-axis predictor and its composition
- [x] girc-core: the rate-distortion margin
- [x] girc-core: the bit-exact decoder round-trip
- [x] girc-bench: the dataset harness, three anchors, the BD-rate
      implementation and the margin sweep
- [x] girc-train: the collection, fit, export and evaluation pipeline

## Product B — Evaluation portal (apps/portal)
- [x] organisation accounts, authentication and roles
- [x] the confidential agreement and its versioned acceptance
- [x] the evaluation request workflow
- [x] the gated artifact delivery
- [x] result submission and validation
- [x] the administrator console and the audit log
- [x] the security fixes: path traversal, the forged audit trail, the
      sign-in rate limit
- [x] the test suite: 19 tests, green

## Websites
- [x] web/girc-site (Product A), published from CI
- [x] web/eval-site (Product B)

## Research package (research/patent-package)
- [x] the technical disclosure
- [x] the claim set, with the per-axis prediction in the independent claim
- [x] the claim chart against the prior art
- [x] the prior-art matrix
- [x] the reproducibility manifest, with the checksums of every artefact

## Verification and ship
- [x] cargo test: 45 tests green
- [x] the portal tests: 19 green
- [x] the benchmark reproduced over twelve sequences, four QPs, three anchors
- [x] the CI: Rust, Python, the transform oracle and the site link check
- [x] the site published at devansh0703.github.io/girc
- [x] the repository topics and the homepage set
- [x] the licence and the notice
- [x] pushed to one repository

## Open, and deliberately not cut
- [~] The mechanism saves 0.226%, and it is smaller on nine of the
      twelve sequences. The predictability ceiling of the joint mode is
      44.98% against a 44.31% majority baseline, and each axis on its
      own reaches 51.75%. The binding constraint is the predictability
      of the mode, not the signalling.
- [ ] A counsel-led prior-art search. The analysis in the patent package
      is a web search, not a freedom-to-operate opinion, and the
      MediaTek reference needs a proper reading of its full text.
- [ ] Inter prediction. The mechanism's behaviour with a
      motion-compensated residual is untested, and it is the condition
      most likely to move the number.
- [ ] A VVC-conformant bitstream. GIRC measures the mechanism with its
      own format, which isolates the signalling and nothing else.
