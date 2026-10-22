# Reproducibility manifest

This manifest records the exact state that produced the numbers in
`research/patent-package/README.md` and on the product site. Every figure
can be traced to the state below.

## The state

| Item | Value |
|---|---|
| Repository commit | `ea8b4b12fee9e7cd7eb2e71c5846dc985c10ca3e` |
| Reference codec | `girc-core` 0.1.0 |
| Benchmark harness | `girc-bench` 0.1.0 |
| Rust toolchain | rustc 1.99.0 (b940084d7 2026-09-28) |
| Python | 3.12.13 |
| VVenC | 1.14.0 (`vvencapp version 1.14.0`) |
| FFmpeg | n9.0.1-11-ge47273f4d9-20260831 |
| x265 | via FFmpeg's `libx265` |
| SVT-AV1 | via FFmpeg's `libsvtav1` |

## The content

The Xiph.Org CIF test sequences, first frame of each.

| File | SHA-256 | Bytes |
|---|---|---|
| `akiyo_cif.y4m` | `0f7a1f997930217d601ab324b1428859fdb599dde8d33772a24bfe2803a9b067` | 45621044 |
| `foreman_cif.y4m` | `b0e3c35eff9fe1da91521a02eb9f0e3ef4bc5aec341ecef1567cd87f1711267b` | 45621044 |
| `mobile_cif.y4m` | `8b32924ea00ef3bd52f7ddafd3ceb55d2bb331610b59957c5c3cc9628f9e0d90` | 45621054 |
| `stefan_cif.y4m` | `f73153ac05ad55be7523f52f2329d278357382f379961c2be67585691cdb8a43` | 13686344 |

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
| Transform set | DCT-II, DCT-VIII, DST-VII (4, 8, 16 and 32 sample sizes implemented) |
| Intra modes | planar, DC and eight angular |
| Planes coded | luma |
| Margin for the published per-sequence table | 10 bits |
| Margin sweep | 0, 2, 5, 10, 20, 50, 100, 500 bits |
| Rate-distortion lambda | 0.85 |
| Predictor | multinomial logistic regression, 24 features, three classes |

## Reproducing

```sh
git checkout ea8b4b12fee9e7cd7eb2e71c5846dc985c10ca3e
cargo build --release --workspace
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
