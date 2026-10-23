# TODOS — GIRC (Geometry-Inference Codec)

Status legend: `[ ]` todo · `[~]` in progress · `[x]` done · `[-]` cut

GIRC is one product with four parts, not two products. The framing that
produced an earlier version of this file — "Product A" the toolkit and
"Product B" the portal, each with a site of its own — was a misreading of the
brief. There is one codec, one training method, one benchmark and one hosted
service, and they are the parts of a single thing: the codec measures, the
pipeline fits the predictor, the harness measures both, and the portal and the
service make it runnable by somebody who did not build it.

## Part 1 — The codec (crates/girc-core, crates/girc-cli)
- [x] VVC integer transforms, bit-exact against VVenC
- [x] the four per-axis multiple transform selection modes, bit-exact
      against VVenC, over all sixteen mode and size combinations
- [x] the mts_idx signalling, matching the standard's syntax
- [x] entropy coding and the bitstream container
- [x] intra prediction and rate-distortion search
- [x] inter prediction with integer-sample motion
- [x] the geometry predictor, with the neighbour-transform context
- [x] the joint-mode predictor: one model over the five modes
- [x] the rate-distortion margin
- [x] the bit-exact decoder round-trip
- [x] the 4:2:0 chroma planes, the chroma quantization parameter mapping,
      and the separate luma and chroma payloads
- [x] the command line: encode, decode, inspect, collect, evaluate

## Part 2 — The training pipeline (python/girc-train)
- [x] the collection pass: the encoder's own decisions, not a proxy
- [x] the fit: one multinomial logistic regression over the joint mode
- [x] the predictability ceiling: a gradient-boosted tree over the same
      features, as the upper bound any model over them can reach
- [x] the frozen, versioned artifact, with the data checksum and the
      backend that produced it
- [x] the evaluation report: the per-class accuracy, the ceiling and the
      majority baseline, published together

## Part 3 — The benchmark (crates/girc-bench)
- [x] the dataset harness, three anchors, the BD-rate implementation and
      the margin sweep
- [x] the constant predictor measured alongside the learned one, so a
      claim about a trained model is checked against the alternative that
      needs no model
- [x] the whole-frame and luma rate axes, because the mechanism is a luma
      mechanism
- [x] the report's own tool versions, so a number traces to its state

## Part 4 — The portal and the service (apps/portal)
- [x] organisation accounts, authentication and roles
- [x] the confidential agreement and its versioned acceptance
- [x] the evaluation request workflow
- [x] the gated artifact delivery, signed and bound to the digest
- [x] result submission and validation
- [x] the administrator console and the audit log
- [x] the security fixes: path traversal, the forged audit trail, the
      sign-in rate limit
- [x] the test suite: 24 tests, green
- [x] the hosted analysis endpoint: the same code, over HTTP, on one free
      instance
- [x] the render manifest, so what is deployed is in the repository

## The site (web/girc-site)
- [x] one site, not two: the codec's pages and the evaluation's pages
- [x] light is the default theme, dark is the opt-in, both as custom
      properties in one stylesheet
- [x] every figure generated from a committed artifact by one script, so a
      figure cannot exist without the data behind it
- [x] at least two figures on every page
- [x] no invented address, no placeholder domain, no dark default
- [x] the design contract, checked in CI

## Research package (research/patent-package)
- [x] the technical disclosure
- [x] the claim set, with the joint-mode prediction in the independent
      claim
- [x] the claim chart against the prior art
- [x] the prior-art matrix
- [x] the reproducibility manifest, with the checksums of every artefact
- [x] published in the repository rather than released under an agreement

## Verification and ship
- [x] cargo test: 78 tests green
- [x] the portal tests: 24 green
- [x] the benchmark reproduced over twelve sequences, four QPs, three anchors
- [x] the CI: Rust, Python, the transform oracle, the benchmark-number
      check, the site checks and the design contract
- [x] the site published at devansh0703.github.io/girc
- [x] the repository topics and the homepage set
- [x] the licence and the notice
- [x] pushed to one repository

## Open, and deliberately not cut
- [~] The mechanism saves 0.236% on the luma plane at its best margin,
      on the current model, and the learned predictor beats the constant
      one, at −0.236% against −0.120%. The predictability ceiling of the
      joint mode is 50.65% against a 50.10% majority baseline, so the
      binding constraint is the predictability of the mode, not the
      signalling: the rate-distortion choice between the five modes is
      close to a tie, and there is little for a predictor to predict.
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
- [~] A VVC-conformant bitstream. GIRC measures the mechanism with its
      own format, which isolates the signalling and nothing else. What
      *is* conformant is the signalling the mechanism removes: GIRC's
      explicit mode writes the same bins the reference implementation
      writes for `mts_idx`, checked against the reference encoder's own
      output over 187,834 transform units, in `tools/check_mts_syntax.py`
      and in CI. A whole conformant H.266 bitstream would mean a
      conformant H.266 encoder, which is a different and much larger
      piece of work, and it is the right thing for a codec team with
      one to do: that is what the evaluation is for.
- [x] Chroma. The 4:2:0 chroma planes are coded: the plain DCT-II, as
      H.266 4:2:0 requires; the mapped chroma quantization parameter,
      checked against the reference encoder for every quantization
      parameter; a four by four chroma grid, one block per luma block;
      and the chroma motion derived from the luma motion, as H.266
      does. The luma and chroma payloads are separate, so the luma
      rate and the whole-frame rate can be measured separately, and
      the benchmark reports both. The anchor comparison is now a
      like-for-like one: both sides code all three planes.
- [x] A deployment of the evaluation portal: TLS, a reverse proxy, real
      secret injection, backups and distributed rate limiting. It is in
      `deploy/`, it was brought up against PostgreSQL, and the whole
      lifecycle was exercised in the container: register, sign in,
      reach the dashboard, write the audit log, back it up and restore
      it. The Caddyfile validates, and the proxy chain was run: the
      login page comes back with every security header applied and the
      Server header removed, and the audit log records the client's
      address.
- [x] The hosted service, on Render: one free instance, because a
      workspace gets 750 free hours a month and a month is 730 to 744
      hours, so exactly one always-on free service fits. An UptimeRobot
      monitor pings `/health` every five minutes to keep the instance
      awake, since a free service sleeps after fifteen minutes idle.
      Scheduling lives in GitHub Actions rather than on the host,
      because Render's cron jobs are a paid tier.
