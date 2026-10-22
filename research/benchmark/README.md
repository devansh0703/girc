# The benchmark reports

These are the reports the documents' figures come from. They are
committed because they are the evidence: every number quoted in the
README, the design document, the patent package and both websites is
checked against these files by `tools/verify_benchmark_numbers.py`,
which runs in CI.

## The reports

| File | What it is |
|---|---|
| `report-v7-joint.json` | The full benchmark over the twelve sequences: the explicit mode, the inferred mode with the shipped model, the three anchor encoders, and the margin sweep. |
| `report-constant-comparison.json` | The same content with the constant predictor added, so the learned predictor and the one that needs no training can be compared. |

## What the configuration was

| Parameter | Value |
|---|---|
| Sequences | the twelve Xiph.Org CIF sequences in `data/seq/` |
| Frames per sequence | 10 |
| Quantization parameters | 22, 27, 32, 37 |
| Block size | 8 × 8 |
| Margin | 50, in rate-distortion cost units |
| Model | `models/girc-predictor.json`, version `v7-joint-prediction` |
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
export PATH="$PWD/data/tools/vvenc-src/bin/release-static:$PATH"

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
    --margin-sweep 0,25,50,100,200 \
    --report research/benchmark/report-v7-joint.json \
    --work-dir bench-out/work
```

The constant comparison is the same run with `--compare-constant`.

The report carries the tool versions it ran against, so a re-run that
produced a different figure would show why.
