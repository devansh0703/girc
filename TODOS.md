# TODOS — GIRC (Geometry-Inference Reference Codec)

Status legend: `[ ]` todo · `[~]` in progress · `[x]` done · `[-]` cut

## Product A — GIRC toolkit (crates/girc-*, python/girc-train)
- [ ] girc-core: VVC-spec integer transforms (DCT-II/DST-VII/DCT-VIII)
- [ ] girc-core: entropy coding + bitstream container
- [ ] girc-core: intra prediction + RDO
- [ ] girc-core: geometry-inference transform prediction + override signalling
- [ ] girc-core: bit-exact decoder round-trip
- [ ] girc-bench: dataset harness, anchors (x265/SVT-AV1/VVenC), PSNR, BD-rate
- [ ] girc-train: feature extraction + trained predictor + model export

## Product B — Evaluation portal (apps/portal)
- [ ] org accounts + auth + roles
- [ ] NDA acceptance + versioned agreement
- [ ] evaluation request workflow
- [ ] gated artifact delivery (reference code, model, benchmark data)
- [ ] result submission + validation
- [ ] admin console + audit log

## Websites
- [ ] web/girc-site (Product A)
- [ ] web/eval-site (Product B)

## Research package (research/patent-package)
- [ ] technical disclosure
- [ ] claim set
- [ ] prior-art matrix
- [ ] data-rights records
- [ ] reproducibility manifest

## Verification & ship
- [ ] cargo test green
- [ ] real benchmark numbers reproduced
- [ ] portal E2E green
- [ ] both sites build
- [ ] gstack: cso, qa, health, benchmark, review, land-and-deploy
- [ ] single GitHub repo + push
