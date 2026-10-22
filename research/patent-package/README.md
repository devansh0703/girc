# GIRC — Geometry-Inference Codec

**Patent package: inferred transform-type signalling for video coding**

A reference implementation, a training method, a benchmark harness, a
confidential evaluation portal, and the patent materials for a mechanism
that removes explicit transform-type signalling from a video codec.

The mechanism: instead of spending bits to tell the decoder which transform
was used, the decoder infers it from the geometry it has already
reconstructed, and the encoder spends a single bin only when the inference
was wrong.

---

## 1. The disclosure

### 1.1 The problem

A VVC transform block is coded with one of three transforms: DCT-II,
DCT-VIII or DST-VII. The encoder picks one by rate-distortion search, and
the decoder learns which one from the bitstream. The signalling is pure
overhead: it carries nothing the decoder could not have derived.

The reason it cannot be derived is that the choice looks random. It depends
on the block's own residual, which the decoder has not yet seen. The
encoder looks at the residual; the decoder cannot.

### 1.2 The mechanism

The decoder has reconstructed the picture up to the current block. That
reconstruction carries spatial structure the residual does not determine: a
block in a directional neighbourhood tends to want a directional transform,
and a block in a flat one tends to want the default. That signal is
available on both sides at the same moment, so it can carry the choice.

Three components:

1. **Inference.** The decoder reads a two-dimensional causal window of its
   own reconstruction and predicts the transform from its geometry: the
   gradient orientation field, the per-axis energies, their correlation, the
   intra prediction mode and the quantization parameter.

2. **Signalling.** The encoder spends one bin on whether the inference was
   right. When it was wrong, one further bin selects between the two
   transforms the inference ruled out. An override therefore costs two
   bins, against the explicit mode's two, and the inference costs one.

3. **The margin.** The encoder may take a slightly worse transform than its
   search found, when doing so costs less than the override would. This is
   the mechanism's rate-distortion knob, and it is what makes the mechanism
   work at all. See §1.4.

### 1.3 The distinguishing feature

Prior art on implicit transform selection derives the transform from
*syntax*: the block's size, the intra prediction mode, whether intra
sub-block partitioning is used, or the presence of particular coefficients.
Those are quantities the standard already defines and the decoder already
has.

The mechanism disclosed here differs in two ways that the prior art does
not anticipate:

- **The inference is geometric, not syntactic.** The features are a
  gradient field over the reconstructed causal window. The transform
  depends on the *content's* directional structure, not on a mode index.

- **The encoder has a rate-distortion margin.** The encoder is not
  obliged to signal the transform the search chose; it may take a
  transform the decoder will infer, at a cost bounded in bits of rate. No
  reference found derives the transform by rule and then lets the encoder
  trade distortion against the signalling it would otherwise spend.

### 1.4 What the measurements actually show

The honest result, from the repository's own benchmark:

| Margin (bits) | Mean BD-rate | Prediction mismatch |
|---|---|---|
| 0 | +0.146% | 53.0% |
| 2 | +0.051% | 47.8% |
| 5 | +0.051% | 47.6% |
| **10** | **−0.064%** | 47.3% |
| 20 | −0.006% | 46.8% |
| 50 | +0.216% | 44.9% |
| 100 | +0.222% | 41.2% |
| 500 | +2.425% | 23.9% |

At a margin of 10 bits the inferred mode is 0.064% smaller than the
explicit mode, averaged over four Xiph.Org CIF sequences at QP 22 to 37.
Without a margin the inferred mode is 0.146% *larger*, because the encoder
takes the strictly optimal transform and overrides whenever the inference
differs.

The mechanism is close to break-even, and the reason is measurable rather
than mysterious. The predictability ceiling of the feature set, measured
by fitting a gradient-boosted tree over the same features, is 56.44%
against a 55.53% majority-class baseline. The transform choice is driven
by the block's own residual, and the causal neighbourhood constrains it
only weakly.

The consequence is that with three transform types over a skewed
distribution, the explicit signalling is already nearly free for a
context-adaptive coder to code, and the distortion the inference costs
exceeds the rate it saves.

**This is a negative-leaning result, and it is reported as one.** It is
also the most useful thing the package contains, because it establishes
the mechanism's real envelope and shows a codec team where it would and
would not pay. §1.5 states the conditions under which the trade changes.

### 1.5 Where the mechanism would pay

Each of these is outside what GIRC can test and inside what an evaluation
with a real codec can:

- **A larger transform set.** The overhead removed grows with the number
  of choices to signal and with how evenly they are distributed. Three
  types over a skewed distribution is close to the least favourable case.
- **Inter prediction.** A motion-compensated residual carries the motion
  vector's structure, which is more directional and more predictable from
  the causal neighbourhood than an intra residual.
- **Coarser signalling grain.** One transform per region rather than per
  block makes the inference easier and the overhead larger.
- **A richer causal window.** The features read a 16-sample window. A
  larger one, or a learned representation of the neighbourhood, would
  raise the predictability ceiling.

---

## 2. Prior art

The following were identified and reviewed. Each is mapped to the
disclosure in §3.

| Reference | Assignee | Priority | What it discloses | Distinction |
|---|---|---|---|---|
| US 16/815,920 / HK40058354A, "Implicit transform selection in video coding" | Qualcomm | 2019-03-12 | Derives the transform type from the block's size and whether intra sub-block partitioning is used | Syntactic derivation; no geometry; no rate-distortion margin |
| EP19305698.3 / EP3977732B1, "Implicit multiple transform selection" | InterDigital | 2019-07-26 | Derives the horizontal and vertical transform type from the block's width and height by table lookup; reports **+0.34% overall** against an MTS-disabled anchor | Syntactic derivation; no geometry; no margin; and its own reported result is a loss |
| US 17/066,791 / US11785254B2, "Implicit mode dependent primary transforms" | Tencent America | 2021-01-21 | Derives the primary transform from the intra prediction mode and the block size | Syntactic derivation; no geometry; no margin |
| US 16/646,624 / US11363302B2, "Method and apparatus of neural network for video coding" | MediaTek | 2019-03-27 | Uses a neural network to decide the multiple core transforms for a block, from the reconstructed block | A learned predictor exists, but the input is the current block's own reconstruction and the transform is not derived from a *causal* neighbourhood with a rate-distortion margin |
| US12143605, "Inter-prediction Mode-dependent Transforms for Video Coding" | — | — | Derives transform modes for inter blocks from side information | Syntactic derivation |
| EP3843395A1, "Transform selection in a video encoder and/or video decoder" | — | — | Improved process for selecting the transform set | Selection process; no causal geometric inference |
| US12368889B2, "Coefficient-based transform and mode signaling" | — | — | Modifies MTS to skip transform signalling under end-of-block conditions | Rule-based skipping, not geometric inference |
| TW202131697A, "Multiple transform set signaling for video coding" | — | — | Derives the MTS index by a predetermined rule when only the DC coefficient is non-zero | Rule-based derivation |

The two closest references are the InterDigital and the MediaTek
applications. The distinction from each is stated in §3.

**This is an exploratory search, not a freedom-to-operate opinion.** It was
conducted with web search against public patent databases. A counsel-led
search across Espacenet, the USPTO and the EPO registers, in the
jurisdictions that matter, has not been performed and is required before
any filing.

---

## 3. Claim chart

The claims are drafted to survive the prior art in §2. They are a
starting point for counsel, not a filing-ready set.

### Claim 1 — a method in a video encoder

> A method in a video encoder for coding a current block of a video
> picture, the method comprising:
>
> (a) determining, from a causal neighbourhood of already-reconstructed
>     samples of the video picture, a plurality of geometric features
>     comprising at least a gradient orientation distribution over the
>     neighbourhood;
> (b) applying a trained prediction function to the features to obtain a
>     predicted transform type from a plurality of candidate transform
>     types;
> (c) determining a candidate transform type for the current block by
>     rate-distortion optimisation over the candidate transform types;
> (d) comparing the candidate transform type with the predicted transform
>     type and, when the difference in rate-distortion cost between them
>     is within a margin expressed in units of rate, selecting the
>     predicted transform type in place of the candidate transform type;
> (e) generating a coded representation of the current block using the
>     selected transform type; and
> (f) writing to the coded representation a signal indicating whether the
>     selected transform type is the predicted transform type, and, when
>     it is not, an indication of which of the candidate transform types
>     other than the predicted transform type was selected.

### Claim 2 — the features

> The method of claim 1, wherein determining the features comprises:
> computing a two-dimensional gradient field over the causal
> neighbourhood; and forming the gradient orientation distribution by
> accumulating gradient magnitudes into bins according to the gradient
> orientation, the bins covering the full circle.

### Claim 3 — the causal neighbourhood

> The method of claim 1, wherein the causal neighbourhood is a window
> ending at the top-left corner of the current block, every sample of
> which has been reconstructed before the current block is coded.

### Claim 4 — the signal

> The method of claim 1, wherein the signal indicating which of the
> candidate transform types was selected is a single bin selecting between
> the candidate transform types other than the predicted transform type, in
> a canonical order.

### Claim 5 — the prediction function

> The method of claim 1, wherein the prediction function is a multinomial
> logistic regression over the features, and wherein the prediction
> function is trained on transform types selected by rate-distortion
> optimisation during encoding of a training set.

### Claim 6 — a method in a video decoder

> A method in a video decoder for decoding a current block of a video
> picture, the method comprising:
>
> (a) reconstructing a causal neighbourhood of already-reconstructed
>     samples of the video picture;
> (b) determining, from the causal neighbourhood, a plurality of geometric
>     features comprising at least a gradient orientation distribution over
>     the neighbourhood;
> (c) applying a trained prediction function to the features to obtain a
>     predicted transform type from a plurality of candidate transform
>     types;
> (d) reading from the coded representation a signal indicating whether
>     the predicted transform type was used;
> (e) when the signal indicates that the predicted transform type was
>     used, transforming the current block using the predicted transform
>     type; and
> (f) when the signal indicates that the predicted transform type was not
>     used, reading an indication of which of the candidate transform types
>     other than the predicted transform type was used, and transforming
>     the current block using that transform type.

### Claim 7 — training

> A method of training a prediction function for use in the encoder or
> decoder of claim 1 or claim 6, the method comprising:
>
> (a) encoding a training set of pictures with a reference encoder, the
>     reference encoder performing rate-distortion optimisation over the
>     candidate transform types;
> (b) for each block of the training set, recording the geometric features
>     of the block's causal neighbourhood as reconstructed by the
>     reference encoder, and the transform type the reference encoder
>     selected for the block; and
> (c) fitting the prediction function to the recorded features and
>     transform types.

### Claim 8 — the margin

> The method of claim 1, wherein the margin is a function of a measured
> rate-distortion cost difference converted into a rate difference through
> the rate-distortion Lagrange multiplier, so that the margin is
> expressed in the same units as the signalling it avoids.

### Distinctions, claim by claim

| Claim | Closest reference | Why the reference does not anticipate the claim |
|---|---|---|
| 1 | EP3977732B1 (InterDigital) | The reference derives the transform from the block's width and height by table lookup. Claim 1 derives it from a gradient orientation distribution over reconstructed samples, and adds the rate-distortion margin of element (d), which the reference has no counterpart for. |
| 1 | US11363302B2 (MediaTek) | The reference's neural network takes the current block's own reconstruction as input, which the decoder does not have at the point the transform is needed. Claim 1's features come from a *causal* neighbourhood that excludes the current block, and the reference has no margin. |
| 2 | any | No reference found forms a gradient orientation distribution over a causal window. |
| 3 | US12143605 | That reference uses side information associated with the prediction, not a spatial window of reconstructed samples. |
| 4 | any | No reference found codes a correction relative to a predicted value in a canonical order. |
| 5 | US11363302B2 | The reference trains a network, but not on the encoder's own rate-distortion decisions, and its output is not a transform type chosen by a margin. |
| 6 | EP3977732B1 | The reference's decoder derives the transform from syntax alone. Claim 6's decoder derives it from geometry and resolves a mismatch against a canonical ordering. |
| 7 | US11363302B2 | The reference does not disclose collecting the encoder's own rate-distortion decisions over a causal neighbourhood. |
| 8 | none found | No reference found expresses the signalling-versus-distortion trade as a rate-denominated margin. |

---

## 4. Prior art — the novelty risk, stated plainly

Three facts a counsel will want to hear from the inventor rather than
discover later.

**The concept of implicit transform selection is not new.** Four families
of reference do it. A claim whose novelty rests on "the transform type is
derived rather than signalled" would not survive §2. The claims above are
drafted to rest on the geometric features, the causal neighbourhood and
the rate-distortion margin instead.

**The measured result is close to break-even, and the closest prior art
reports a loss.** InterDigital's EP3977732B1 reports +0.34% overall for
its implicit transform design against an MTS-disabled anchor: their
approach also did not pay. The present disclosure's −0.064% at a margin of
10 bits is a slightly better result by a slightly different mechanism,
and both are far from a decisive win. A filing that claimed a large
coding gain would be contradicted by the reference implementation in this
repository.

**The negative result is itself the package's strongest asset.** A
mechanism whose envelope has been measured — the margin at which it pays,
the predictability ceiling of its features, the content on which it helps
and the content on which it does not — is a much more useful evaluation
offer than a claimed win that a codec team has to rediscover the
boundaries of. The evaluation portal is built to collect exactly that
evidence.

---

## 5. What the package contains

| Artefact | Path | What it is |
|---|---|---|
| Reference codec | `crates/girc-core` | VVC-exact integer transforms, intra prediction, quantization, a context-adaptive entropy coder, and both signalling modes |
| Command-line interface | `crates/girc-cli` | `encode`, `decode`, `inspect`, `collect`, `evaluate` |
| Benchmark harness | `crates/girc-bench` | Both modes and three anchor codecs, the Bjontegaard implementation, and the margin sweep |
| Training pipeline | `python/girc-train` | Collects the encoder's own decisions, fits the predictor, reports the predictability ceiling |
| Evaluation portal | `apps/portal` | Organisation accounts, versioned agreement, gated delivery, results, audit log |
| Product site | `web/girc-site` | The public description of the mechanism and the results |
| Evaluation site | `web/eval-site` | The public description of the evaluation |
| Reproducibility manifest | `research/patent-package/REPRODUCIBILITY.md` | How to reproduce every number in this document |

---

## 6. Reproducing the numbers

```sh
# Build the reference codec.
cargo build --release --workspace

# Train the predictor on the training sequences.
PYTHONPATH=python/girc-train python3 -m girc_train collect \
    --sequences data/seq/*.y4m --output bench-out/training.jsonl
PYTHONPATH=python/girc-train python3 -m girc_train fit \
    --input bench-out/training.jsonl --output models/girc-predictor.json

# Run the benchmark, including the margin sweep.
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

The exact tool versions and the checksum of the training data are recorded
in the report the harness writes, so a number can be traced to the state
that produced it.

---

## 7. The evaluation

The commercial form of this work is a confidential evaluation offered to
codec teams: the reference implementation, the trained predictor, the
benchmark harness and the patent materials, released under a confidential
evaluation agreement through the portal in `apps/portal`.

The portal's security properties are tested rather than asserted: the
agreement gates the downloads, a download token is signed and bound to
the artifact's digest so it cannot be edited or replayed, one
organisation's results cannot be submitted against another's request, and
every state change — including failed sign-ins — reaches an append-only
audit log.

A licence or an assignment follows the evaluation rather than preceding
it.

---

## 8. Provenance of the reference arithmetic

The transform implementation is not an approximation. The integer
transforms are bit-exact against Fraunhofer HHI's VVenC 1.14.0, checked by
a test that drives the reference implementation's own kernels and compares
the results bit for bit across all twelve transform and size
combinations. The quantization follows ITU-T H.266 clause 8.6.3 with
VVC's `g_quantScales` table and the transform shift that accounts for the
widened dynamic range.

The test sequences are the Xiph.Org CIF set, distributed under a licence
that permits their use in codec research.

---

## 9. Filing notes

- **A counsel-led search is required.** §2 is a web search. It is
  indicative, not a freedom-to-operate opinion.
- **The jurisdiction matters.** A US application, a European application
  and an EP validation have different prior-art cut-off dates. The
  InterDigital family is active in Europe, so the European position is
  the one to establish first.
- **The claims may be better framed as a method of reducing signalling
  overhead** than as a specific transform-inference scheme, which is
  broader and harder to design around.
- **The negative result should be disclosed.** A package that states its
  own limits is more credible to a codec company than one that does not,
  and the evaluation is more likely to succeed for it.
- **The MediaTek reference needs a proper reading.** Its neural network
  takes the current block's reconstruction, which the decoder does not
  have; that is the distinction, and it needs to be established against
  the full text rather than the abstract.
