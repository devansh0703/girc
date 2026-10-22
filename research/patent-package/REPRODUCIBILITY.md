# Reproducibility manifest

This manifest records the exact state that produced the numbers in
`research/patent-package/README.md` and on the product site. Every figure
can be traced to the state below.

## The state

| Item | Value |
|---|---|
| Repository commit | the commit that adds this manifest's own update; `git log -1` gives the exact one |
| Reference codec | `girc-core` 0.1.0 |
| Benchmark harness | `girc-bench` 0.1.0 |
| Rust toolchain | rustc 1.99.0 (b940084d7 2026-09-28) |
| Python | 3.12.13 |
| VVenC | 1.14.0 (`vvencapp version 1.14.0`) |
| FFmpeg | n9.0.1-11-ge47273f4d9-20260831 |
| x265 | via FFmpeg's `libx265` |
| SVT-AV1 | via FFmpeg's `libsvtav1` |
| Sequences | twelve Xiph.Org CIF sequences, fetched by `tools/fetch_sequences.sh` |

## The content

The Xiph.Org CIF test sequences, first frame of each. They are fetched
by `tools/fetch_sequences.sh`, which prints their checksums, and they
are not in the repository because of their size.

| File | Bytes |
|---|---|
| `akiyo_cif.y4m` | 45621044 |
| `bus_cif.y4m` | 22810538 |
| `city_cif.y4m` | 45621038 |
| `coastguard_cif.y4m` | 45621044 |
| `container_cif.y4m` | 45621044 |
| `crew_cif.y4m` | 45621038 |
| `deadline_cif.y4m` | 208944224 |
| `flower_cif.y4m` | 38017544 |
| `foreman_cif.y4m` | 45621044 |
| `highway_cif.y4m` | 304140044 |
| `mobile_cif.y4m` | 45621054 |
| `stefan_cif.y4m` | 13686344 |

## The artefacts

| Artefact | SHA-256 |
|---|---|
| `models/girc-predictor.json` | `902829903847361262121e6a4622961f5310b55a681cd6aa3d182159f4252b35` |
| `bench-out/training.jsonl` | `90e30044ebd9329ddd83b001bfc1e9e4ec0a1b04b1de2e48a90746610ddd0990` |
| `bench-out/report.json` | `5f98f845cadd87ec23afacee1c11ace7b44ef68f83963a85b0be62d01edaf000` |

The training data's checksum is also recorded inside the model file, so a
model can be checked against the set it was fitted on without this
document.

## The configuration

| Parameter | Value |
|---|---|
| Quantization parameter sweep | 22, 27, 32, 37 |
| Block size | 8 × 8 |
| Transform modes | the plain DCT-II and the four per-axis combinations of DCT-VIII and DST-VII that ITU-T H.266 defines (4, 8, 16 and 32 sample sizes implemented, bit-exact against VVenC) |
| Intra modes | planar, DC and eight angular |
| Planes coded | luma |
| Margin for the published per-sequence table | 50 bits |
| Margin sweep | 0, 2, 5, 10, 20, 50, 100, 500 bits |
| Rate-distortion lambda | 0.85 |
| Predictor | two multinomial logistic regressions, 46 features each, one per axis, composed into the mode |

## Reproducing

```sh
./tools/fetch_sequences.sh
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
    --anchors x265,svt-av1,vvenc \
    --margin-sweep 0,2,5,10,20,50,100,500 \
    --report bench-out/report.json
```

The training set is rebuilt the same way:

```sh
for sequence in data/seq/*.y4m; do
  for qp in 22 27 32 37; do
    ./target/release/girc collect --input "$sequence" --qp "$qp"
  done
done > bench-out/training.jsonl

PYTHONPATH=python/girc-train python3 -m girc_train fit \
  --input bench-out/training.jsonl \
  --output models/girc-predictor.json
```

The harness records the tool versions it ran against inside the report,
so a re-run that produced a different figure would show why.

## What is not reproduced

The following are deliberately outside the measurement, and a reader should
not infer anything about them from these numbers:

- **Chroma.** GIRC codes the luma plane only. The Y4M chroma planes are
  read and written, but they are not coded.
- **Inter prediction.** GIRC is intra-only.
- **More than one frame.** The harness codes the first frame of each
  sequence. The anchors are held to the same frame.
- **A VVC-conformant bitstream.** GIRC's format is its own. It is not
  comparable with a VVC decoder, and the anchor comparison is context
  rather than a claim of competitiveness.
- **The transform sizes other than 8 × 8.** The transform arithmetic is
  implemented and tested for 4, 8, 16 and 32 samples, but the codec codes
  8 × 8 blocks only.

## Verification the repository performs itself

These are checks that run with `cargo test --workspace` and
`pytest apps/portal/tests`:

| Check | What it establishes |
|---|---|
| `transform::tests::matches_vvc_reference_implementation` | The forward and inverse transforms are bit-exact against VVenC's own kernels, over all twelve transform and size combinations |
| `codec::tests::coefficient_magnitudes_round_trip` | Every residual magnitude from 0 to 4095 survives the binarisation |
| `codec::tests::decode_reproduces_the_encoder_reconstruction` | The decoder reproduces the encoder's reconstruction exactly, in both signalling modes |
| `bitstream::tests::the_context_tracks_the_observed_rate` | The entropy coder's context estimator converges to the observed rate rather than drifting |
| `quant::tests::the_reconstruction_error_is_bounded_by_the_step` | The quantizer's reconstruction is within one step at every quantization parameter |
| `girc-bench`'s `identical_curves_have_zero_bd_rate` and its siblings | The Bjontegaard implementation is checked against synthetic curves with known answers |
| The portal's twelve tests | The agreement gates the downloads, a download token cannot be edited or replayed, results cannot be cross-submitted, and every state change is audited |
