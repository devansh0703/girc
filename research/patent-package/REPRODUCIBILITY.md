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

| File | SHA-256 | Bytes |
|---|---|---|
| `akiyo_cif.y4m` | `0f7a1f997930217d601ab324b1428859fdb599dde8d33772a24bfe2803a9b067` | 45621044 |
| `bus_cif.y4m` | `08f557aade483465fca3ad9f5ce9a5b6aad7e833c871ec969b5b8fbb0a791c87` | 22810538 |
| `city_cif.y4m` | `2f99e08e66376c0a0acc9c7349aef7f61020be8fbfe6440bdb0c7ec7d05bb30f` | 45621038 |
| `coastguard_cif.y4m` | `ae4052051ee9d77236ced5b231239dec97831c66faea3d7b70694e222ce00c43` | 45621044 |
| `container_cif.y4m` | `45abe3a99a0e2914742b3eecf474335362e31921bc0b30eef1dd45bb42df82be` | 45621044 |
| `crew_cif.y4m` | `c838aac518b44aa5d6be2c68d904a836f641c74481b4fcef611809cd31691b14` | 45621038 |
| `deadline_cif.y4m` | `4498852bf8f18d0c56957a4eff9611e3a851bfa9087e6ac8efffdadf2d02b964` | 208944224 |
| `flower_cif.y4m` | `26c7934b77186836a39db113e8011e841bd593e37295efb98a38915b24aa2b8e` | 38017544 |
| `foreman_cif.y4m` | `b0e3c35eff9fe1da91521a02eb9f0e3ef4bc5aec341ecef1567cd87f1711267b` | 45621044 |
| `highway_cif.y4m` | `a2a96ed29fc21e352353a042aea945f610772db8a69fab9ccf97d70e29e16d0a` | 304140044 |
| `mobile_cif.y4m` | `8b32924ea00ef3bd52f7ddafd3ceb55d2bb331610b59957c5c3cc9628f9e0d90` | 45621054 |
| `stefan_cif.y4m` | `f73153ac05ad55be7523f52f2329d278357382f379961c2be67585691cdb8a43` | 13686344 |

## The artefacts

| Artefact | SHA-256 |
|---|---|
| `models/girc-predictor.json` | `000334ebf25c89b7d6764430dcfefcf8019786a9bf00b1aa378f8daa65fdcc78` |
| `bench-out/training.jsonl` | `26046d242f7161f62d47871b49889e1e2355373f0e1863570ad99311df480445` |
| `bench-out/report.json` | `b4b23f1a6a65d3b6e6ba57b97d93efe506d83653120875839a1d75de87c31b42` |
| `bench-out/fair/report-10f.json` | the whole-frame benchmark over the twelve sequences at ten frames, with the anchors |

The training data's checksum is also recorded inside the model file, so a
model can be checked against the set it was fitted on without this
document.

## The configuration

| Parameter | Value |
|---|---|
| Quantization parameter sweep | 22, 27, 32, 37 |
| Block size | 8 × 8 |
| Transform modes | the plain DCT-II and the four per-axis combinations of DCT-VIII and DST-VII that ITU-T H.266 defines (4, 8, 16 and 32 sample sizes implemented, bit-exact against VVenC) |
| Intra modes | planar, DC and eight angular on the luma plane; planar, the two axis-aligned directions and DC on the chroma planes |
| Planes coded | luma, and the two 4:2:0 chroma planes |
| Chroma transform | the plain DCT-II, as ITU-T H.266 4:2:0 requires; no chroma transform type is signalled |
| Chroma quantization parameter | the luma one mapped through H.266's chroma quantization parameter mapping, checked against VVenC for every quantization parameter |
| Chroma block grid | four by four chroma samples, one chroma block per luma block |
| Chroma motion | derived from the luma motion, as H.266 does; no chroma motion bits of its own |
| Frames per sequence | 10 |
| Margin for the published per-sequence table | 50, in rate-distortion cost units |
| Margin sweep | 0, 25, 50, 100, 200, in rate-distortion cost units |
| Rate-distortion lambda | 0.85 |
| Predictor | one multinomial logistic regression over the joint mode, 64 features |

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
    --frames 10 \
    --anchors x265,svt-av1,vvenc \
    --margin-sweep 0,25,50,100,200 \
    --report bench-out/report.json
```

The report carries two mechanism numbers, and they answer different
questions. `mechanism_bd_rate` is on the luma plane alone, and it is the
mechanism's own effect: the mechanism changes the luma signalling and
nothing else, so the luma rate is the axis its effect is measured on.
`mechanism_bd_rate_total` includes the chroma planes, and it is what a
deployment would actually save. The anchor comparison is on the
whole-frame rate, because the anchors code all three planes.

## The result

At a margin of 50, over the twelve sequences:

| Measure | Value | Better on |
|---|---|---|
| Mean BD-rate, luma plane | −0.239% | 10 of 12 |
| Mean BD-rate, whole frame | −0.156% | 10 of 12 |
| Median BD-rate, luma plane | −0.332% | — |
| Best sequence | `deadline_cif`, −0.902% | — |
| Worst sequence | `akiyo_cif`, +1.403% | — |
| Prediction mismatch | 42.5% | — |

The learned predictor against the constant one, which is the
alternative that needs no training and no model:

| Predictor | Mean BD-rate, luma | Better on |
|---|---|---|
| Constant, always the plain DCT-II | −0.120% | 10 of 12 |
| The shipped learned model | −0.239% | 10 of 12 |

The margin sweep, on the luma plane:

| Margin | Mean BD-rate | Mismatch | Better on |
|---|---|---|---|
| 0 | −0.056% | 46.4% | 12 of 12 |
| 25 | −0.138% | 44.7% | 9 of 12 |
| 50 | −0.239% | 42.5% | 10 of 12 |
| 100 | +0.116% | 38.0% | 8 of 12 |
| 200 | +0.557% | 30.7% | 5 of 12 |

The training set is rebuilt the same way:

```sh
mkdir -p bench-out/train-pred
for sequence in data/seq/*.y4m; do
  for qp in 22 27 32 37; do
    ./target/release/girc collect --input "$sequence" --qp "$qp" --frames 10
  done
done > bench-out/train-pred/mixed.jsonl

PYTHONPATH=python/girc-train python3 -m girc_train fit \
  --input bench-out/train-pred/mixed.jsonl \
  --output models/girc-predictor.json \
  --model-version v7-joint-prediction
```

That produces 760,320 records, and the model records the SHA-256 of the
data it was fitted on, so a re-run that produced different data would
be visible:

```
sha256  247e62742c56ead475195cf9dbebb9043b9308608274f8a75213d7fd9d2ef229
```

The data itself is 422 MB and is not in the repository; the collection
is deterministic, because the encoder's search is, so re-running the
command above reproduces it. The fitted model is in the repository, and
`crates/girc-core/tests/shipped_model.rs` checks that it loads, that
its weights match the codec's feature count, and that it is not the
constant predictor.

The harness records the tool versions it ran against inside the report,
so a re-run that produced a different figure would show why.

## What is not reproduced

The following are deliberately outside the measurement, and a reader should
not infer anything about them from these numbers:

- **Sub-sample motion.** The motion search is whole-sample only, and the
  chroma motion is the luma motion halved and rounded to a whole chroma
  sample. H.266 has an eighth-sample chroma interpolation filter.
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
| `chroma_qp::tests::the_table_matches_the_reference_encoder` | The chroma quantization parameter mapping is exactly the one VVenC derives, for every quantization parameter |
| `tools/check_mts_syntax.py` | GIRC's transform-type signalling writes the same bins the reference implementation writes, for all five modes, checked against the reference encoder's own output |
| `chroma::tests::every_chroma_intra_mode_survives_the_code` | Every chroma intra mode survives the truncated unary code |
| `chroma::tests::a_chroma_plane_round_trips` | A chroma plane round-trips through the coder exactly, and its inter blocks are predicted from the luma motion |
| `codec::tests::a_frame_round_trips` | A 4:2:0 frame round-trips exactly in all three planes, and the chroma is a real reconstruction rather than a flat fill |
| `codec::tests::the_chroma_planes_cost_bits` | The chroma planes cost bits, so the total rate is a real whole-frame rate |
| `girc-bench`'s `identical_curves_have_zero_bd_rate` and its siblings | The Bjontegaard implementation is checked against synthetic curves with known answers |
| The portal's nineteen tests | The agreement gates the downloads, a download token cannot be edited or replayed, results cannot be cross-submitted, and every state change is audited |
