# The benchmark reports

These are the reports the documents' figures come from. They are
committed because they are the evidence: every number quoted in the
README, the design document, the patent package and both websites is
checked against these files by `tools/verify_benchmark_numbers.py`,
which runs in CI.

## The reports

| File | What it is |
|---|---|
| `report-v9-joint.json` | The full benchmark over the twelve sequences: the explicit mode, the inferred mode with the shipped model, the constant predictor, the three anchor encoders, and the margin sweep. |
| `transform-spread.json` | How close the transform choice is to a tie: the rate-distortion gap between the best mode and the runner-up, per block, on akiyo_cif at QP 27. The figure's own premise-check. |
| `inter-predictability.json` | The intra/inter study: one model fit per axis on the intra blocks and one on the inter blocks, each against its own majority baseline. |

The two summaries are committed next to the report they feed, because
the figures read them at build time and a fresh clone must be able to
produce every figure. Their heavy inputs — the encoder's block records,
hundreds of megabytes of jsonl — stay in the gitignored `bench-out/`.

## Reproducing the two summaries

```sh
# the transform-search spread; the tool hardcodes QP 27, the benchmark's mid QP
cargo build --release -p girc-bench --bin transform-spread
./target/release/transform-spread data/seq/akiyo_cif.y4m \
    research/benchmark/transform-spread.json

# the intra/inter study; needs the collected training records and scikit-learn
PYTHONPATH=python/girc-train python3 tools/inter_predictability.py \
    bench-out/training-mixed.jsonl \
    research/benchmark/inter-predictability.json
```

## What the configuration was

| Parameter | Value |
|---|---|
| Sequences | the twelve Xiph.Org CIF sequences in `data/seq/` |
| Frames per sequence | 10 |
| Quantization parameters | 22, 27, 32, 37 |
| Block size | 8 × 8 |
| Margin | 50, in rate-distortion cost units |
| Model | `models/girc-predictor.json`, version `v9-joint-gpu` |
| Anchors | x265, SVT-AV1, VVenC 1.14.0 |

## The two rate axes

The mechanism changes the luma signalling and nothing else, so there
are two figures and they answer different questions:

- `mechanism_bd_rate` is on the luma plane. It is the mechanism's own
  effect.
- `mechanism_bd_rate_total` includes the two chroma planes. It is what
  a deployment would actually save.

The comparison against the anchors is on the whole-frame rate, because
the anchors code all three planes and so does GIRC.

## Reproducing them

```sh
cargo build --release --workspace
source tools/benchmark_env.sh

./target/release/girc-bench \
    --sequence data/seq/akiyo_cif.y4m \
    --sequence data/seq/bus_cif.y4m \
    --sequence data/seq/city_cif.y4m \
    --sequence data/seq/coastguard_cif.y4m \
    --sequence data/seq/container_cif.y4m \
    --sequence data/seq/crew_cif.y4m \
    --sequence data/seq/deadline_cif.y4m \
    --sequence data/seq/flower_cif.y4m \
    --sequence data/seq/foreman_cif.y4m \
    --sequence data/seq/highway_cif.y4m \
    --sequence data/seq/mobile_cif.y4m \
    --sequence data/seq/stefan_cif.y4m \
    --qps 22,27,32,37 \
    --model models/girc-predictor.json \
    --margin 50 \
    --frames 10 \
    --anchors x265,svt-av1,vvenc \
    --compare-constant \
    --margin-sweep 0,25,50,100,200 \
    --report research/benchmark/report-v9-joint.json \
    --work-dir bench-out/work
```

The `source tools/benchmark_env.sh` puts VVenC on the path, and says so
if any of the three anchor encoders is missing.

The report carries the tool versions it ran against, so a re-run that
produced a different figure would show why.
