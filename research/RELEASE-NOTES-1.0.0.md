# GIRC 1.0.0

The Geometry-Inference Codec: a reference implementation, a training
method, a benchmark harness, and a patent package for removing explicit
transform-type signalling from video coding.

## What this is

A video codec usually signals each block's transform type. ITU-T H.266
codes a transform type for the luma plane, and a conformant decoder
reads it. GIRC asks whether the decoder can work it out for itself,
from geometry both sides already have, and what it costs to be wrong.

The answer, measured over twelve Xiph.Org CIF sequences at ten frames
each and four quantization parameters, with the margin at 50:

| Measure | Value | Better on |
|---|---|---|
| Mean BD-rate, luma plane | **−0.239%** | 10 of 12 |
| Mean BD-rate, whole frame | **−0.156%** | 10 of 12 |
| The same, with the constant predictor | −0.120% | 10 of 12 |

The last row is the one that matters most. The constant predictor
always answers the plain DCT-II and needs no training and no model, so
it is the alternative any claim about a trained predictor has to beat.
The learned model is ahead on nine of the twelve sequences, and the gap
between them is 0.119 percentage points of BD-rate, which is twice the
mechanism's own effect.

## What is in the box

**The codec.** VVC's integer transforms, bit-exact against Fraunhofer
HHI's VVenC 1.14.0 for all sixteen per-axis mode and size
combinations. Intra prediction with ten modes, inter prediction with
integer-sample motion, the 4:2:0 chroma planes with the chroma
quantization parameter mapping the standard defines, VVC-style
quantization, and a context-adaptive entropy coder. The transform type
is coded with the syntax H.266 uses for `mts_idx`, checked against the
reference encoder's own output over 187,834 transform units.

**The mechanism.** The decoder predicts the transform mode from the
reconstructed causal neighbourhood, the decoded neighbours' own
transforms, the motion context and the block's own prediction plane. The
encoder accepts the prediction when it is within a rate-distortion
margin of the best mode, and sends a correction relative to the
prediction only when it is not. The margin is the part of this that the
prior art does not reach.

**The training method.** The predictor is a multinomial logistic
regression over the joint mode, fitted on 760,320 real block records
from the twelve sequences, exported as a frozen versioned artifact so
the encoder and the decoder are guaranteed bit-identical. The training
data's SHA-256 is recorded in the model.

**The benchmark.** A harness that runs GIRC in both signalling modes,
sweeps the margin, and measures against x265, SVT-AV1 and VVenC on the
same content with the same code. The reports are committed in
`research/benchmark/`, and the figures the documents quote are checked
against them by a script that runs in CI.

**The patent package.** A claim-level prior-art search over ten
publications, with the claim text extracted and read, the elements
charted, and every quotation machine-checked against the extracted text.
The result: the rate-distortion margin is the only element that no
reference reaches.

**The evaluation portal.** A confidential portal for gated delivery,
with an append-only audit log, agreement gates, signed download tokens,
path confinement, sign-in throttling shared across processes, and a
production deployment in `deploy/` with TLS, secret injection,
backups and a restore that cannot half-apply.

**Two websites.** The codec's public site, and the evaluation site's
public pages.

## Getting it

```sh
git clone https://github.com/devansh0703/girc
cd girc
cargo build --release --workspace
./target/release/girc encode --input data/seq/akiyo_cif.y4m --output out.girc
./target/release/girc decode --input out.girc --output out.y4m
```

The test suite needs the reference encoder to check the transforms
against:

```sh
git clone --depth 1 --branch v1.14.0 \
    https://github.com/fraunhoferhhi/vvenc.git data/tools/vvenc-src
cmake -S data/tools/vvenc-src -B data/tools/vvenc-src/build \
    -DCMAKE_BUILD_TYPE=Release
cmake --build data/tools/vvenc-src/build --config Release -j "$(nproc)"
./tools/generate_transform_oracle.sh
./tools/generate_chroma_qp_oracle.sh
cargo test --workspace
```

The sequences come from Xiph.Org, and `tools/fetch_sequences.sh` gets
them and records their hashes.

## What it is not

It is not a VVC competitor, and it does not claim to be. GIRC is a
research codec with a small mode set, integer-sample motion and no
sub-sample interpolation, and the anchor comparison is context rather
than a claim.

It is not a conformant H.266 bitstream. The signalling it removes is
the standard's, and that is checked, but a conformant bitstream would
mean a conformant encoder.

It is not a freedom-to-operate opinion. The search is a public-record
search, and a patent attorney has to make the call.

It is not a large win. The mechanism saves about a quarter of one
percent, and the reason is measurable rather than mysterious: the
transform mode is close to a tie in rate-distortion terms, so there is
little for a predictor to predict. An evaluation with a richer residual
coder is where the mechanism should pay.

## Licence

Apache-2.0. See `LICENSE` and `NOTICE`.
