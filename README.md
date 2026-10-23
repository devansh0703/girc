# GIRC — Geometry-Inference Codec

A reference codec that replaces explicit transform-type signalling with an
inference the decoder performs from its own reconstruction, plus the training
method, the benchmark harness, the hosted analysis and the published patent
materials that go with it.

The mechanism: instead of spending bits to tell the decoder which transform
was used, the decoder infers it from the geometry it has already
reconstructed, and the encoder spends a single bin only when the inference
was wrong.

**One product, four ways in.** The codec you build and run yourself. The
training pipeline that fits the predictor on the encoder's own decisions. The
benchmark that measures both, against three anchor codecs. And a hosted
analysis service that runs the measurement so you do not need a toolchain
first.

| | |
|---|---|
| **Site** | [devansh0703.github.io/girc](https://devansh0703.github.io/girc/) |
| **Service** | [girc-portal.onrender.com](https://girc-portal.onrender.com/) |
| **Source** | [github.com/devansh0703/girc](https://github.com/devansh0703/girc) |
| **Report** | [`research/REPORT.md`](research/REPORT.md) — every figure, generated |
| **Licence** | Apache-2.0, the whole of the code |
| **Maintainer** | [devanshraulo@gmail.com](mailto:devanshraulo@gmail.com) |

## The result, stated honestly

Over twelve Xiph.Org CIF sequences, ten frames each, at QP 22 to 37, on the
current model:

| Margin (RD-cost units) | Mean BD-rate, luma | Prediction mismatch | Better on |
|---|---|---|---|
| 0 | −0.056% | 46.4% | 12 of 12 |
| 25 | −0.119% | 44.6% | 10 of 12 |
| 50 | **−0.236%** | 42.6% | 11 of 12 |
| 100 | +0.002% | 37.9% | 10 of 12 |
| 200 | +0.515% | 30.6% | 6 of 12 |

At the margin of 50 the mechanism is **−0.236% on the luma plane** and
**−0.149% on the whole frame**, and it is better on 11 of the 12 sequences on
the luma plane and 10 of the 12 on the whole frame.

**The learned predictor beats the constant one, and the benchmark measures
both.** The constant predictor always answers the plain DCT-II and needs no
training and no model, so it is the alternative any claim about a trained
predictor has to beat:

| Predictor | Mean BD-rate, luma | Better on |
|---|---|---|
| Constant, always the plain DCT-II | −0.120% | 10 of 12 |
| The shipped learned model | **−0.236%** | 11 of 12 |

The learned model is ahead on 6 of the 12 sequences, and the gap between them
is 0.116 percentage points of BD-rate, which is about half the mechanism's own
effect.

Two rate axes are reported because the mechanism changes the luma signalling
and nothing else. The luma rate is the mechanism's own effect, and the
whole-frame rate is what a deployment would actually save. The comparison
against the anchors is on the whole-frame rate, because the anchors code all
three planes and so does GIRC.

The mechanism is a small win, and the reason it is not a larger one is
measurable rather than mysterious. The shipped model predicts the mode
correctly on 50.17% of the held-out blocks, against a 50.10% majority class.
The predictability ceiling of the feature set, measured by fitting a
gradient-boosted tree over the same features, is 50.65%. The joint mode is
close to unpredictable: the rate-distortion choice between the modes is close
to a tie, so there is little for a predictor to predict. The per-class accuracy
of the shipped model is 91.6% on the plain DCT-II and between 5.4% and 15.2%
on the four multiple transform selection modes.

That measurement is the mechanism's most useful result. It says the constraint
is not the signalling and not the coding tools: it is that the transform mode
is a near-tie in rate-distortion terms. An evaluation with a richer residual
coder, where the modes separate more, is where the mechanism should pay.

The closest prior art reports the same kind of result. InterDigital's
EP3977732B1 reports **+0.34% overall** for its implicit transform design,
against an MTS-disabled anchor.

**The rate-distortion margin is the only part of the disclosure that the prior
art does not reach.** The claim text of ten references was extracted and read,
and the findings are: E1 implicit derivation in 10, E2 reconstructed-neighbourhood geometry in 3, E3 per-axis selection in 6, E4 the rate-distortion margin in 0, E5 trained prediction in 2, E6 prediction-relative correction in 2. No claim in any of them reads a threshold,
a margin, a sub-optimal choice, or a trade of distortion against signalling.
The search, the claim charts and the quotations are in
[the prior art report](research/patent-package/PRIOR_ART.md), and every
quotation in it, and every cell of its result table, is checked against the
extracted claim text by the test suite.

## What is here

| Path | What it is |
|---|---|
| `crates/girc-core` | The codec: VVC-exact integer transforms, intra and inter prediction, 4:2:0 chroma, quantization, a context-adaptive entropy coder, and both signalling modes |
| `crates/girc-cli` | `encode`, `decode`, `inspect`, `collect`, `evaluate` |
| `crates/girc-bench` | The benchmark: both modes and three anchor codecs, the Bjontegaard implementation, and the margin sweep |
| `python/girc-train` | The training pipeline: collects the encoder's own decisions, fits the joint-mode predictor, reports the predictability ceiling |
| `apps/portal` | The evaluation portal and the hosted analysis service |
| `deploy` | The portal's production deployment, the hosted service's container and manifest, and the Render deployment |
| `web/girc-site` | The product site, at [devansh0703.github.io/girc](https://devansh0703.github.io/girc) |
| `research/patent-package` | The disclosure, the claim chart, the prior-art analysis, the reproducibility manifest |
| `research/benchmark` | The committed benchmark reports the documents' figures are checked against |
| `research/REPORT.md` | The report: every figure, generated from those artifacts |
| `tools/` | The reference encoders' oracles, the claim-chart verifier, the benchmark-number verifier, the figure generator, the site assembler, the design contract |

## Use it

```sh
# 1. build the codec
git clone https://github.com/devansh0703/girc
cd girc
cargo build --release --workspace

# 2. or skip the build entirely and post a sequence to the hosted analysis
curl -sS -o report.json \
  -F "sequence=@foreman_cif.y4m" \
  "https://girc-portal.onrender.com/analyze?format=json"
```

The hosted analysis runs the same codec the repository ships, on one free
Render instance, and stores nothing: the sequence lives in the request and is
dropped with it. Its limits — an 8 MiB body, 64 frames, 36,000 luma samples,
a 60 s budget, one request at a time — are published on its own `/health`
rather than discovered by being rejected. The sample limit is the instance's,
not the method's: the full report is twenty-one codec runs, and that is what
fits one shared core inside the budget. The command line has no such limit.

## Building

```sh
cargo build --release --workspace
cargo test --workspace
```

The test suite checks the integer transforms against Fraunhofer HHI's VVenC bit
for bit, across all twelve transform and size combinations, and checks the
chroma quantization parameter mapping against the same reference implementation
for every quantization parameter. It needs the reference implementation:

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
encoder's actual behaviour and not a proxy for it, and the features are taken
from the reconstructed neighbourhood exactly as the decoder will see them.

The fit runs on the GPU when one is present and on the CPU when it is not, and
the two agree on the result: the same data gives 50.17% test accuracy on the
GPU and 50.18% on the CPU, and the exported weights have the same shape, so
the codec's loader cannot tell which produced them. The GPU path is an
optional extra, not a requirement:

```sh
uv pip install --python .venv/bin/python -r python/girc-train/requirements-gpu.txt
```

The model records which backend produced it, in
`training.estimator_backend`, so a fit is attributable. The predictability
ceiling is a gradient-boosted tree, which stays on the CPU because cuML has no
histogram gradient boosting.

## The chroma planes

GIRC codes 4:2:0 content: the luma plane and both chroma planes. That is not a
detail — the anchor codecs code all three planes, so comparing a luma-only
GIRC rate against a whole-frame anchor rate was not a comparison of the same
thing.

What the standard fixes, and what the codec therefore does:

- **No chroma transform type is signalled.** H.266 clause 7.3.9 codes a
  transform type for the luma plane; the chroma transform type is inferred,
  and in 4:2:0 it is always the plain DCT-II. The reference encoder agrees:
  chroma transform selection is gated on `m_useChromaTS`, which is off by
  default and only enabled for 4:4:4.
- **So the mechanism is a luma mechanism** — exactly where the standard signals
  a transform type at all. It removes all of the transform-type signalling
  there is, and nothing else.
- **The chroma quantization parameter is mapped, not equal**, through H.266's
  chroma quantization parameter mapping, derived the way the reference encoder
  derives it and checked against it for every quantization parameter.
- **The chroma grid is four by four chroma samples**, one chroma block per luma
  block, and the chroma motion is derived from the luma motion as H.266 does.

The luma and chroma payloads are separate and the container records each one's
length, because the mechanism changes the luma signalling and nothing else. The
benchmark therefore reports two mechanism numbers: the effect on the luma
plane, which is the mechanism's own effect, and the effect on the whole frame,
which is what a deployment would actually save. The anchor comparison uses the
whole frame, on both sides.

## The evaluation portal and the hosted service

The portal is the confidential evaluation environment: organisation accounts, a
versioned agreement, gated artifact delivery, result submission and an
append-only audit log. The hosted analysis is the other half: post a sequence
and get the same figures the benchmark writes.

The portal's production deployment is in `deploy/`. It is three containers:
Caddy in front, terminating TLS and setting the security headers; the portal
itself on uvicorn; and PostgreSQL, which is not published to the host at all.
The artifacts are on their own volume.

The secret is not in the image. It is passed in at run time through a mode-600
environment file that is not in the repository. The sign-in rate limit is kept
in Redis, so it holds across every worker rather than one.

The backup script dumps the database with `pg_dump` — including the audit log,
which is the record of who was given what — archives the artifacts, writes a
manifest, and verifies the backup against it before calling the run a success.
The restore script cannot half-apply: it drops the database, loads the dump
into a new one, and empties the artifacts volume before unpacking into it.

`deploy/README.md` has the whole procedure, including how to rotate the
signing secret.

The hosted analysis runs on Render as one free service, because a workspace
gets 750 free instance hours a month and a month is 730 to 744 hours, so
exactly one always-on free service fits. A free instance sleeps after fifteen
minutes idle, so an UptimeRobot monitor pings `/health` every five minutes.
Render's cron jobs are a paid tier, so the scheduled canary lives in GitHub
Actions rather than on the host. `render.yaml` is in the repository, so what
is deployed can be compared with what is committed.

```sh
PYTHONPATH=apps/portal uvicorn girc_eval.service:app --factory --port 8000
```

The portal's own deployment is unchanged:

```sh
python -m venv .venv && source .venv/bin/activate
pip install -r apps/portal/requirements.txt
PYTHONPATH=apps/portal uvicorn girc_eval.app:create_app --factory --port 8000
```

The tests:

```sh
PYTHONPATH=apps/portal pytest apps/portal/tests -q
```

They check the things that matter: that the agreement gates the downloads, that
a download token cannot be edited to fetch a different artifact or replayed
after it expires, that one organisation's results cannot be submitted against
another's request, that every state change reaches the append-only audit log,
and that the hosted analysis publishes the limits it enforces and refuses a
body that is not a sequence rather than reporting a measurement of nothing.

## Regenerating the figures

Every figure on the site and in the report is generated from a committed
artifact, so you can check that a figure you are reading is the one your run
produces:

```sh
.venv/bin/python tools/generate_figures.py
.venv/bin/python tools/generate_report.py
.venv/bin/python tools/assemble_site.py
.venv/bin/python tools/verify_benchmark_numbers.py \
    research/benchmark/report-v9-joint.json
```

The last command reads the benchmark report and checks that every figure the
documents quote is the one the report holds. It has caught a number changed in
one place and not in the others, which is exactly the drift a project whose
value is checkable claims cannot afford.

## What this is not

- **It is not a VVC decoder.** GIRC's bitstream is its own research format. It
  exists to measure one mechanism with everything else held constant.
- **It is not a VVC competitor.** GIRC is a research codec with a small mode
  set, integer-sample motion and no sub-sample interpolation. The anchor
  comparison is context, and it is on the whole-frame rate, with both sides
  coding all three planes.
- **It is not a claim of a large coding gain.** The measured result is close to
  break-even, and the package says so.
- **It is not closed source and it is not paid.** The code, the training
  pipeline, the benchmark, the portal and the patent materials are all in this
  repository under Apache-2.0, and the hosted analysis is free to use. The
  patent materials are published rather than released under an agreement,
  because a claim is only worth as much as somebody else's ability to check it.
- **The prior art is real.** Four families of reference already disclose
  implicit transform selection. The claims are drafted around the geometric
  features, the causal neighbourhood and the rate-distortion margin, and a
  counsel-led search is still required.

## Licence

The code in this repository is released under the Apache Licence 2.0; see
[`LICENSE`](LICENSE). The patent rights are not granted by that licence, and
the patent materials in `research/patent-package/` are published for
examination rather than licensed.
