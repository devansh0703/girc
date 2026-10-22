# GIRC — Geometry-Inference Codec

A reference implementation, a training method, a benchmark harness, a
confidential evaluation portal, and the patent materials for a mechanism
that removes explicit transform-type signalling from a video codec.

The mechanism: instead of spending bits to tell the decoder which
transform was used, the decoder infers it from the geometry it has already
reconstructed, and the encoder spends a single bin only when the inference
was wrong.

**Start here:** [`research/patent-package/README.md`](research/patent-package/README.md)
— the disclosure, the prior art, the claim chart, and the measured
results.

---

## What is here

| Path | What it is |
|---|---|
| `crates/girc-core` | The codec: VVC-exact integer transforms, intra and inter prediction, 4:2:0 chroma, quantization, a context-adaptive entropy coder, and both signalling modes |
| `crates/girc-cli` | `encode`, `decode`, `inspect`, `collect`, `evaluate` |
| `crates/girc-bench` | The benchmark: both modes and three anchor codecs, the Bjontegaard implementation, and the margin sweep |
| `python/girc-train` | The training pipeline: collects the encoder's own decisions, fits the joint-mode predictor, reports the predictability ceiling |
| `apps/portal` | The confidential evaluation environment |
| `deploy` | The evaluation portal's production deployment: TLS, secret injection, backups, a restore that cannot half-apply |
| `web/girc-site` | The product site, at [devansh0703.github.io/girc](https://devansh0703.github.io/girc) |
| `web/eval-site` | The evaluation site, at [/evaluate/](https://devansh0703.github.io/girc/evaluate/) |
| `research/patent-package` | The disclosure, the claim chart, the prior-art analysis, the reproducibility manifest |
| `research/benchmark` | The committed benchmark reports the documents' figures are checked against |
| `tools/` | The reference encoders' oracles, the claim-chart verifier, the benchmark-number verifier, the syntax checker |

## The result, stated honestly

The mechanism removes the explicit signalling of the transform mode and
replaces it with the decoder's own prediction of it. Over twelve
Xiph.Org CIF sequences, ten frames each, at QP 22 to 37, on the
current model:

| Margin (RD-cost units) | Mean BD-rate, luma | Prediction mismatch | Better on |
|---|---|---|---|
| 0 | −0.056% | 46.4% | 12 of 12 |
| 25 | −0.119% | 44.6% | 10 of 12 |
| 50 | **−0.236%** | 42.6% | 11 of 12 |
| 100 | +0.002% | 37.9% | 10 of 12 |
| 200 | +0.515% | 30.6% | 6 of 12 |

At the margin of 50 the mechanism is **−0.236% on the luma plane** and
**−0.149% on the whole frame**, and it is better on 11 of the 12
sequences on the luma plane and 10 of the 12 on the whole frame.

**The learned predictor beats the constant one, and the benchmark
measures both.** The constant predictor always answers the plain DCT-II
and needs no training and no model, so it is the alternative any claim
about a trained predictor has to beat:

| Predictor | Mean BD-rate, luma | Better on |
|---|---|---|
| Constant, always the plain DCT-II | −0.120% | 10 of 12 |
| The shipped learned model | **−0.236%** | 11 of 12 |

The learned model is ahead on 6 of the 12 sequences, and the gap
between them is 0.116 percentage points of BD-rate, which is about half
the mechanism's own effect.

Two rate axes are reported because the mechanism changes the luma
signalling and nothing else. The luma rate is the mechanism's own
effect, and the whole-frame rate is what a deployment would actually
save. The comparison against the anchors is on the whole-frame rate,
because the anchors code all three planes and so does GIRC.

The mechanism is a small win, and the reason it is not a larger one is
measurable rather than mysterious. The shipped model predicts the mode
correctly on 50.17% of the held-out blocks, against a 50.10% majority
class. The predictability ceiling of the feature set, measured by
fitting a gradient-boosted tree over the same features, is 50.65%. The
joint mode is close to unpredictable: the rate-distortion choice between
the modes is close to a tie, so there is little for a predictor to
predict. The per-class accuracy of the shipped model is 91.6% on the
plain DCT-II and between 5.3% and 15.4% on the four multiple transform
selection modes.

Those figures are low, and they are low for an honest reason that is
worth stating. An inter block carries a motion vector instead of an
intra mode, so the decoder cannot know the mode the encoder would have
chosen for it. A feature built from that mode would let the encoder
predict from information the decoder does not have, and a model trained
on it would look better than it could ever be. The earlier figures of
53.4% were flattered by exactly that, and the fix is in the shipped
code: the intra mode feature is empty for an inter block, and the
benchmark decodes what it encodes and fails if the two pictures differ.

That measurement is the mechanism's most useful result. It says the
constraint is not the signalling and not the coding tools: it is that
the transform mode is a near-tie in rate-distortion terms. An
evaluation with a richer residual coder, where the modes separate more,
is where the mechanism should pay.

The closest prior art reports the same kind of result. InterDigital's
EP3977732B1 reports **+0.34% overall** for its implicit transform
design, against an MTS-disabled anchor.

**The rate-distortion margin is the only part of the disclosure that
the prior art does not reach.** The claim text of ten references was
extracted and read, and the findings are:
E1 implicit derivation in 10, E2 reconstructed-neighbourhood geometry in 3, E3 per-axis selection in 6, E4 the rate-distortion margin in 0, E5 trained prediction in 2, E6 prediction-relative correction in 2.
No claim in any of them reads a threshold, a margin, a sub-optimal choice,
or a trade of distortion against signalling. The search, the claim charts
and the quotations are in
[the prior art report](research/patent-package/PRIOR_ART.md), and every
quotation in it, and every cell of its result table, is checked against
the extracted claim text by the test suite.

## The chroma planes

GIRC codes 4:2:0 content: the luma plane and both chroma planes. That
is not a detail — the anchor codecs code all three planes, so comparing
a luma-only GIRC rate against a whole-frame anchor rate was not a
comparison of the same thing.

What the standard fixes, and what the codec therefore does:

- **No chroma transform type is signalled.** H.266 clause 7.3.9 codes a
  transform type for the luma plane; the chroma transform type is
  inferred, and in 4:2:0 it is always the plain DCT-II. The reference
  encoder agrees: chroma transform selection is gated on
  `m_useChromaTS`, which is off by default and only enabled for 4:4:4.
- **So the mechanism is a luma mechanism** — exactly where the standard
  signals a transform type at all. It removes all of the transform-type
  signalling there is, and nothing else.
- **The chroma quantization parameter is mapped, not equal**, through
  H.266's chroma quantization parameter mapping, derived the way the
  reference encoder derives it and checked against it for every
  quantization parameter.
- **The chroma grid is four by four chroma samples**, one chroma block
  per luma block, and the chroma motion is derived from the luma motion
  as H.266 does.

The luma and chroma payloads are separate and the container records
each one's length, because the mechanism changes the luma signalling
and nothing else. The benchmark therefore reports two mechanism
numbers: the effect on the luma plane, which is the mechanism's own
effect, and the effect on the whole frame, which is what a deployment
would save. The anchor comparison uses the whole frame, on both sides.

## The evaluation portal, in production

The portal is in `deploy/`. It is three containers: Caddy in front,
terminating TLS and setting the security headers; the portal itself
on uvicorn; and PostgreSQL, which is not published to the host at all.
The artifacts are on their own volume.

The secret is not in the image. It is passed in at run time through a
mode-600 environment file that is not in the repository. The sign-in
rate limit is kept in Redis, so it holds across every worker rather
than one.

The backup script dumps the database with `pg_dump` — including the
audit log, which is the record of who was given what — archives the
artifacts, writes a manifest, and verifies the backup against it before
calling the run a success. The restore script cannot half-apply: it
drops the database, loads the dump into a new one, and empties the
artifacts volume before unpacking into it.

`deploy/README.md` has the whole procedure, including how to rotate
the signing secret.

## Building

```sh
cargo build --release --workspace
cargo test --workspace
```

The test suite checks the integer transforms against Fraunhofer HHI's
VVenC bit for bit, across all twelve transform and size combinations,
and checks the chroma quantization parameter mapping against the same
reference implementation for every quantization parameter. It needs
the reference implementation:

```sh
git clone --depth 1 --branch v1.14.0 \
    https://github.com/fraunhoferhhi/vvenc.git data/tools/vvenc-src
cmake -S data/tools/vvenc-src -B data/tools/vvenc-src/build \
    -DCMAKE_BUILD_TYPE=Release
cmake --build data/tools/vvenc-src/build --config Release -j "$(nproc)"
./tools/generate_transform_oracle.sh
./tools/generate_chroma_qp_oracle.sh
cargo test -p girc-core transform
cargo test -p girc-core chroma_qp
```

## The evaluation portal

```sh
python -m venv .venv && source .venv/bin/activate
pip install -r apps/portal/requirements.txt
PYTHONPATH=apps/portal uvicorn girc_eval.app:create_app --factory --port 8000
```

The tests:

```sh
PYTHONPATH=apps/portal pytest apps/portal/tests -q
```

They check the things that matter for a confidential portal: that the
agreement gates the downloads, that a download token cannot be edited to
fetch a different artifact or replayed after it expires, that one
organisation's results cannot be submitted against another's request, and
that every state change reaches the append-only audit log.

## The training pipeline

```sh
pip install -r python/girc-train/requirements.txt
export PYTHONPATH=python/girc-train

python3 -m girc_train collect --sequences data/seq/*.y4m \
    --output bench-out/training.jsonl
python3 -m girc_train fit --input bench-out/training.jsonl \
    --output models/girc-predictor.json
```

The collection pass runs the encoder itself, so the training set is the
encoder's actual behaviour and not a proxy for it, and the features are
taken from the reconstructed neighbourhood exactly as the decoder will see
them.

The fit runs on the GPU when one is present and on the CPU when it is
not, and the two agree on the result: the same data gives 50.17% test
accuracy on the GPU and 50.18% on the CPU, and the exported weights have
the same shape, so the codec's loader cannot tell which produced them. The GPU path is an
optional extra, not a requirement:

```sh
uv pip install --python .venv/bin/python -r python/girc-train/requirements-gpu.txt
```

The model records which backend produced it, in
`training.estimator_backend`, so a fit is attributable. The
predictability ceiling is a gradient-boosted tree, which stays on the
CPU because cuML has no histogram gradient boosting.

## The benchmark

```sh
export PATH="$PWD/data/tools/vvenc-src/bin/release-static:$PATH"

./target/release/girc-bench \
    --sequence data/seq/akiyo_cif.y4m \
    --sequence data/seq/foreman_cif.y4m \
    --sequence data/seq/mobile_cif.y4m \
    --sequence data/seq/stefan_cif.y4m \
    --qps 22,27,32,37 \
    --model models/girc-predictor.json \
    --margin 10 \
    --anchors x265,svt-av1,vvenc \
    --margin-sweep 0,2,5,10,20,50,100,500 \
    --report bench-out/report.json
```

The report records the tool versions it ran against, so a number can be
traced to the state that produced it.

## What this is not

- **It is not a VVC decoder.** GIRC's bitstream is its own research
  format. It exists to measure one mechanism with everything else held
  constant.
- **It is not a VVC competitor.** GIRC is a research codec with a small
  mode set, integer-sample motion and no sub-sample interpolation. The
  anchor comparison is context, and it is on the whole-frame rate, with
  both sides coding all three planes.
- **It is not a claim of a large coding gain.** The measured result is
  close to break-even, and the package says so.
- **The prior art is real.** Four families of reference already disclose
  implicit transform selection. The claims are drafted around the
  geometric features, the causal neighbourhood and the rate-distortion
  margin, and a counsel-led search is still required.

## Licence

The code in this repository is released under the Apache Licence 2.0; see
[`LICENSE`](LICENSE). The patent rights are not granted by that licence,
and the evaluation agreement in `apps/portal/girc_eval/agreement.py` is
the instrument that governs the confidential material.
