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
- [~] The mechanism saves about 0.1% at its best margin, on the current
      model. The predictability ceiling of the joint mode is 44.98%
      against a 44.31% majority baseline, and each axis on its own
      reaches 51.75%. The binding constraint is the predictability of
      the mode, not the signalling.
- [~] Inter prediction is implemented and measured. The inter blocks'
      mismatch rate is consistently lower than the intra blocks', so
      the hypothesis holds, but the gain over the majority baseline is
      smaller for the inter blocks than for the intra ones, because
      the inter blocks' transform distribution is more concentrated. The
      number is in the patent package.
- [x] A claim-level prior-art search. The claim text of ten references
      was extracted from the publications themselves and read, the
      chart records the claim language for each element, and every
      quotation in the chart and in the report is checked against the
      extracted text by the test suite. The result: only the
      rate-distortion margin is undisclosed.
- [ ] Counsel. A public-record search is not a freedom-to-operate
      opinion, and the national phase filings of the families in the
      set were not searched. That needs a patent attorney.
- [ ] A VVC-conformant bitstream. GIRC measures the mechanism with its
      own format, which isolates the signalling and nothing else.
- [ ] Chroma. GIRC codes the luma plane; the chroma planes are carried
      by the Y4M layer and not coded.
- [ ] A deployment of the evaluation portal: TLS, a reverse proxy, real
      secret injection, backups and distributed rate limiting.

