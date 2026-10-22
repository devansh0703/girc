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
| `crates/girc-core` | The codec: VVC-exact integer transforms, intra prediction, quantization, a context-adaptive entropy coder, and both signalling modes |
| `crates/girc-cli` | `encode`, `decode`, `inspect`, `collect`, `evaluate` |
| `crates/girc-bench` | The benchmark: both modes and three anchor codecs, the Bjontegaard implementation, and the margin sweep |
| `python/girc-train` | The training pipeline: collects the encoder's own decisions, fits the predictor, reports the predictability ceiling |
| `apps/portal` | The confidential evaluation environment |
| `web/girc-site` | The product site |
| `web/eval-site` | The evaluation site |
| `research/patent-package` | The disclosure, the claim chart, the prior-art analysis, the reproducibility manifest |
| `tools/` | The transform-table extractor and the VVenC oracle |

## The result, stated honestly

At a margin of 10 bits, the inferred mode is **0.064% smaller** than the
explicit mode, averaged over four Xiph.Org CIF sequences at QP 22 to 37.
Without a margin it is 0.146% *larger*.

The mechanism is close to break-even, and the reason is measurable: with
three transform types over a skewed distribution, the explicit signalling
is already nearly free for a context-adaptive coder, and the distortion
the inference costs exceeds the rate it saves. The predictability ceiling
of the feature set is 56.44% against a 55.53% majority-class baseline.

The closest prior art reports the same kind of result — InterDigital's
EP3977732B1 reports +0.34% overall for its implicit transform design.
The full discussion, and the conditions under which the trade changes, are
in the [patent package](research/patent-package/README.md).

## Building

```sh
cargo build --release --workspace
cargo test --workspace
```

The transform test suite checks the integer transforms against Fraunhofer
HHI's VVenC bit for bit, across all twelve transform and size
combinations. It needs the reference implementation:

```sh
git clone --depth 1 --branch v1.14.0 \
    https://github.com/fraunhoferhhi/vvenc.git data/tools/vvenc-src
cmake -S data/tools/vvenc-src -B data/tools/vvenc-src/build \
    -DCMAKE_BUILD_TYPE=Release
cmake --build data/tools/vvenc-src/build --config Release -j "$(nproc)"
./tools/generate_transform_oracle.sh
cargo test -p girc-core transform
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
- **It is not a VVC competitor.** GIRC is an intra-only research codec
  with a small mode set. The anchor comparison is context.
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
