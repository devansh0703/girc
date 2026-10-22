# TODOS — GIRC (Geometry-Inference Codec)

Status legend: `[ ]` todo · `[~]` in progress · `[x]` done · `[-]` cut

## Product A — GIRC toolkit (crates/girc-*, python/girc-train)
- [x] girc-core: VVC-spec integer transforms (DCT-II/DST-VII/DCT-VIII)
- [x] girc-core: entropy coding + bitstream container
- [x] girc-core: intra prediction + RDO
- [x] girc-core: geometry-inference transform prediction + override signalling
- [x] girc-core: bit-exact decoder round-trip
- [x] girc-bench: dataset harness, anchors (x265/SVT-AV1/VVenC), PSNR, BD-rate
- [x] girc-train: feature extraction + trained predictor + model export

## Product B — Evaluation portal (apps/portal)
- [x] org accounts + auth + roles
- [x] NDA acceptance + versioned agreement
- [x] evaluation request workflow
- [x] gated artifact delivery (reference code, model, benchmark data)
- [x] result submission + validation
- [x] admin console + audit log
- [x] portal test suite (12 tests, green)

## Websites
- [x] web/girc-site (Product A) — overview, method, results, package
- [x] web/eval-site (Product B) — overview, process, scope

## Research package (research/patent-package)
- [x] technical disclosure
- [x] claim set (8 claims, with a claim chart against the prior art)
- [x] prior-art matrix (8 references, dated, mapped to the claims)
- [x] data-rights records (Xiph.Org test sequences, VVenC/BSD provenance)
- [x] reproducibility manifest (checksums of every artefact and sequence)

## Verification & ship
- [x] cargo test green (47 tests)
- [x] real benchmark numbers reproduced (4 sequences, 4 QPs, 3 anchors)
- [x] portal E2E green (12 tests)
- [x] both sites build and their links resolve
- [x] CI workflow: rust, python, transform oracle, link check
- [x] LICENSE (Apache-2.0) and NOTICE (third-party provenance)
- [x] single GitHub repo + push

## Open, and deliberately not cut
- [~] The mechanism is close to break-even (-0.064% at a margin of 10
      bits). The predictability ceiling of the feature set is 56.44%
      against a 55.53% majority baseline, so the inference is weakly
      determined by the causal neighbourhood. This is reported as the
      result, not hidden.
- [ ] A counsel-led prior-art search. The analysis in the patent package
      is a web search, not a freedom-to-operate opinion, and the
      MediaTek reference needs a proper reading of its full text.
- [ ] The evaluation is only ever run against a codec that is not GIRC,
      so the mechanism's behaviour with a larger transform set, with
      inter prediction, or with coarser signalling grain is untested.
