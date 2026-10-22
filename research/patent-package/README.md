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

A VVC transform block is coded with one of five *modes*: the plain DCT-II,
or one of the four per-axis combinations of DCT-VIII and DST-VII that
ITU-T H.266 clause 8.6.3 defines as multiple transform selection. The
encoder picks one by rate-distortion search, and the decoder learns which
one from the bitstream, through the `mts_idx` syntax element. The signalling
is pure overhead: it carries nothing the decoder could not have derived.

The reason it cannot be derived is that the choice looks random. It depends
on the block's own residual, which the decoder has not yet seen. The
encoder looks at the residual; the decoder cannot.

The mode set is VVC's, not this package's. A mechanism that removed
signalling of a single transform applied to both axes would be measuring
something the standard does not do, and any claim drafted against it would
be a claim about the wrong syntax element.

### 1.2 The mechanism

The decoder has reconstructed the picture up to the current block. That
reconstruction carries spatial structure the residual does not determine: a
block in a directional neighbourhood tends to want a directional transform,
and a block in a flat one tends to want the default. That signal is
available on both sides at the same moment, so it can carry the choice.

Three components:

1. **Inference.** The decoder reads a two-dimensional causal window of its
   own reconstruction and predicts the *mode* from its geometry: the
   gradient orientation field, the per-axis energies, their correlation, the
   intra prediction mode, the quantization parameter, and the modes the
   already-coded neighbouring blocks used.

2. **Per-axis prediction.** The mode is a pair of per-axis transforms, and
   the two are predicted independently and composed. This is the part of
   the mechanism that makes it work, and §1.4 gives the measurement: the
   joint mode is barely predictable, because the rate-distortion choice
   between the modes is close to a tie, while each axis on its own is
   markedly more predictable.

3. **Signalling.** The encoder spends one bin on whether the inference was
   right. When it was wrong, two further bins select between the four
   modes the inference ruled out. The explicit mode codes the mode with
   the standard's own `mts_idx` syntax, which is a flag bin followed by a
   truncated unary code and therefore two to five bins.

4. **The margin.** The encoder may take a slightly worse mode than its
   search found, when doing so costs less than the override would. This is
   the mechanism's rate-distortion knob, and it is what makes the mechanism
   work at all. See §1.4.

### 1.3 The distinguishing feature

The claim text of the ten references in the prior-art search was
extracted and read, and the analysis is in
[`PRIOR_ART.md`](PRIOR_ART.md) with the supporting quotations in
`claim_charts.json`. Every quotation in both is checked against the
extracted claim text by `tools/verify_claim_charts.py`.

The prior art derives the transform from *syntax*: the block's size,
the intra prediction mode, whether intra sub-block partitioning is
used, the presence of particular coefficients, or the transform modes
of a plurality the decoder maintains. Those are quantities the
standard already defines and the decoder already has.

The findings are narrower than they first looked, and the disclosure
has to be narrowed to match:

- **The inference is geometric, not syntactic.** The features are a
  structure tensor over the reconstructed causal window, and the
  transform depends on the *content's* directional structure, not on a
  mode index. EP2991352A1 and US9357221B2 (Thomson, priority 2009) claim
  a transform set "derived from reconstructed video data" and "using
  training data", so a claim to reading reconstructed data is not
  available. US20240040151A1 (Apple, priority 2022) claims the
  transform type inferred from the decoded coefficients' pattern, which
  is the closest art to the geometric reading, though its claims do not
  reach the neighbouring blocks and only its description does.

- **The encoder has a rate-distortion margin.** The encoder is not
  obliged to signal the mode the search chose; it may take a mode the
  decoder will infer, at a cost bounded in bits of rate. No claim in
  any of the ten references reads a threshold, a margin, a sub-optimal
  choice, or a trade of distortion against signalling. US9357221B2
  claim 20 is the closest art, and it signals the transform only when
  the decoder's plurality does not already hold it, which is a
  different decision rule: there the default is free, whereas here the
  encoder takes the prediction when it is within a margin of the best
  mode.

- **The prediction is over the joint mode.** This is the most crowded
  element. US11785254B2 claim 1 claims "a plurality of pairs of
  transform types, each of the plurality of pairs including a vertical
  transform type and a horizontal transform type", and EP2991352A1
  claim 4 claims that "horizontal and vertical directions of the
  transforms are capable of being different". A claim to predicting
  the transform pair at all is not available. What may be available is
  the specific prediction: a *learned* model over the joint mode, its
  features, and the margin applied to the predicted mode.

  The implementation predicts the joint mode with one model rather
  than two per-axis models composed together, and that was a measured
  decision rather than a preference. Each axis on its own was well
  above the majority class — about 54.6% against a 49.8% baseline —
  and the composed mode came out *below* the majority, at 48.3%. The
  five modes are not a product space: the plain DCT-II appears on an
  axis only in the mode where both axes are the plain DCT-II, and the
  four multiple transform selection modes are correlated. Composing
  two independent per-axis models therefore cannot recover the joint
  distribution, and the measurement showed it losing. One model over
  the five modes is both simpler and better.

- **The mechanism replaces exactly the signalling the standard has.**
  ITU-T H.266 clause 7.3.9 codes a transform type for the luma plane;
  the chroma transform type is inferred, and in 4:2:0 the inferred
  chroma transform type is always the plain DCT-II. The reference
  encoder agrees: chroma transform selection is gated on
  `m_useChromaTS`, which defaults to off and is only enabled for 4:4:4
  content, and its intra search assigns `MTS_DCT2_DCT2` to the chroma
  planes directly. So in the format the mechanism targets, the luma
  transform type is the only transform type that is signalled at all,
  and the mechanism removes all of it, and nothing else. This bounds
  the invention precisely: it is a mechanism for removing luma
  transform-type signalling, and it says nothing about chroma because
  the standard says nothing about chroma.

### 1.4 What the measurements actually show

The result, from the repository's own benchmark, over twelve Xiph.Org CIF
sequences, ten frames each, at QP 22 to 37, with the shipped model and
a margin of 50:

| Measure | Value |
|---|---|
| Mean BD-rate, luma plane | −0.239% |
| Mean BD-rate, whole frame | −0.156% |
| Better than the explicit mode | 10 of the 12 sequences, on both axes |

**The learned predictor has to beat the constant one, and it does.** The
constant predictor always answers the plain DCT-II and needs no
training and no model, so it is the alternative any claim about a
trained predictor has to beat. The benchmark runs both over the same
content, the same quantization parameters and the same margin, and
reports them in the same file:

| Predictor | Mean BD-rate, luma | Better on |
|---|---|---|
| Constant, always the plain DCT-II | −0.120% | 10 of 12 |
| The shipped learned model | **−0.239%** | 10 of 12 |

The learned model is ahead on nine of the twelve sequences, and the gap
between the two is 0.119 percentage points of BD-rate, which is twice
the mechanism's own effect.

Two rate axes are reported because the mechanism changes the luma
signalling and nothing else: the luma rate is the mechanism's own
effect, and the whole-frame rate is what a deployment would save. The
comparison against the anchors is on the whole-frame rate, because the
anchors code all three planes and so does GIRC.

The mechanism is a small win, and the reason it is not a larger one is
measurable rather than mysterious. The predictability ceiling of the
feature set, measured by fitting a gradient-boosted tree over the same
features, is 53.9% against a 53.5% majority class. The joint mode is
close to unpredictable: the per-class accuracy of the shipped model is
92.3% on the plain DCT-II and between 3.6% and 17.9% on the four
multiple transform selection modes. The rate-distortion choice between
the modes is close to a tie — the gap between the best and the
second-best mode is under 10% of the distortion for half the blocks —
and so the mode is hard to predict from anything the decoder has.

**That is the mechanism's most useful result.** It says the constraint
is not the signalling and not the coding tools: it is that the
transform mode is a near-tie in rate-distortion terms, so there is
little for a predictor to predict. An evaluation with a richer
residual coder, where the modes separate more, is where the mechanism
should pay, and §1.5 says where that is.

Two findings shaped the implementation, and both are in the repository
rather than in this document:

- **The predictor has to model the joint mode, not the two axes.** An
  earlier version modelled each axis separately and composed them,
  because each axis on its own was well above the majority class — about
  54.6% against 49.8% — and the composed mode came out *below* it, at
  48.3%. The five modes are not a product space: the plain DCT-II
  appears on an axis only in the mode where both axes are the plain
  DCT-II, and the four multiple transform selection modes are
  correlated. Composing two independent per-axis models cannot recover
  the joint distribution, and the measurement showed it losing. One
  model over the five modes is both simpler and better.
- **The prediction's own orientation is the strongest feature.** The
  encoder chooses the transform that best compacts the residual, and
  the residual inherits the prediction's directional structure, so the
  prediction's orientation is the most direct evidence of the transform
  the encoder will want. Both sides have the prediction before they need
  the transform type, because the intra mode and the motion vector are
  both decoded first. For an inter block the prediction comes from the
  previous frame at a motion-compensated position, so it carries
  information the causal window of the current frame does not have.

Even so, the saving is 0.239% and not a headline number, and the report
says so. The explicit signalling is already cheap for a context-adaptive
coder to code, and the mechanism removes an overhead that is close to
the distortion it costs to remove it. §1.5 states the conditions under
which the trade changes.

### 1.5 Where the mechanism would pay

Each of these is outside what GIRC can test and inside what an evaluation
with a real codec can:

- **A larger transform set.** The overhead removed grows with the number
  of choices to signal and with how evenly they are distributed. Five
  modes over a skewed distribution is close to the least favourable case.
- **Coarser signalling grain.** One mode per region rather than per block
  makes the inference easier and the overhead larger.

---

## 2. Prior art

The claim text of the ten references below was extracted from the
publications themselves and read. The full analysis, with the
supporting claim language for each element, is in
[`PRIOR_ART.md`](PRIOR_ART.md) and in `claim_charts.json`, and every
quotation in both is checked against the extracted text by
`tools/verify_claim_charts.py`.

| Reference | Assignee | Priority | What its claims disclose |
|---|---|---|---|
| EP2991352A1, "Methods and apparatus for transform selection in video encoding and decoding" | Thomson Licensing | 2009-01-27 | A transform set "derived from reconstructed video data from one or more previous pictures" and determined "using training data", with "horizontal and vertical directions of the transforms capable of being different", and the transform signalled only above a threshold block size |
| US9357221B2, "Methods and apparatus for adaptive transform selection for video encoding and decoding" | Thomson Licensing | 2009-07-23 | A plurality of transforms adapted from "residue information from the encoded one or more pictures", trained, and the encoder transmits the selected transform "when the selected transform is not present at the corresponding decoder" |
| US11297348B2, "Implicit transform settings for coding a block of pixels" | MediaTek | 2018-04-13 | The transform mode derived from a sub-block transform's partitioning orientation and the transformed sub-block's position |
| EP3977732B1, "Implicit multiple transform selection" | InterDigital | 2019-05-31 | The transform selected from the low frequency non-separable transform mode and the matrix-based intra prediction mode; the description reports **+0.34% overall** against an MTS-disabled anchor |
| US11134275B2, "Method and apparatus for performing primary transform based on filtering of blocks" | Tencent America | 2019-06-04 | The primary transform chosen from the filtering mode and the block's width and height, with the horizontal and the vertical transform selected separately |
| HK40058354A, "Implicit transform selection in video coding" | Qualcomm | 2020-03-12 | The transform type inferred from the block's size and its intra sub-block partitioning, with "selecting the transform type for horizontal use and selecting the transform type for vertical use" |
| US11785254B2, "Implicit mode dependent primary transforms" | Tencent America | 2020-05-29 | A set of "pairs of transform types, each of the plurality of pairs including a vertical transform type and a horizontal transform type", selected implicitly from the intra prediction mode and the block size |
| US12219138B2, "Simplified transform coding tools" | ByteDance | 2020-06-08 | A transform coefficient zero-out based on "an implicit selected transform of a multiple transform selection process", with the horizontal and the vertical matrix derived from the block's own representative coefficients |
| US20230086769A1, "High-level syntax control on implicit transform" | Tencent America | 2021-09-01 | The enabling of an implicit MTS by high-level syntax and the block's prediction mode, with "no transform type information is signaled by the implicit MTS" |
| US20240040151A1, "Coefficient-based transform and mode signaling" | Apple | 2022-07-28 | The transform type inferred from the block's own coefficient pattern, and the transform signalling omitted when the inferred value matches a default |

**US11363302B2 is not transform art.** It is *"Neural post-filter for
video coding"*, and its claims are about a neural network that
generates a filtered reconstruction from a luma and a chroma component.
An earlier table in this project listed it as transform art, and that
was wrong; the relevant MediaTek reference is US11297348B2.

The result of the search is that **only the rate-distortion margin is
undisclosed**. Every reference derives the transform implicitly, four
read reconstructed data, four select the horizontal and the vertical
transform separately, two use training data, and two signal the
transform only when the decoder's own state does not already hold it.
No claim in any of the ten reads a threshold, a margin, a sub-optimal
choice, or a trade of distortion against signalling. The distinction
for each element is stated in §3 and in `PRIOR_ART.md`.

**This is a public-record search, not a freedom-to-operate opinion.** It
was conducted against Google Patents and the EPO's publication server,
reading the claim text of the publications themselves. It did not
search the national phase filings of the families in the set, and it did
not search the non-patent literature. A counsel-led search across
Espacenet, the USPTO and the EPO registers, in the jurisdictions that
matter, is required before any filing.

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
> (b) applying a first trained prediction function to the features to
>     obtain a predicted transform type for a horizontal axis of the
>     current block, and applying a second trained prediction function to
>     the features to obtain a predicted transform type for a vertical
>     axis of the current block, the first and second prediction functions
>     being separately trained;
> (c) combining the predicted transform types of the two axes to obtain a
>     predicted transform mode, in which a predicted DCT-II on either axis
>     yields a predicted transform mode of the plain DCT-II on both axes,
>     and otherwise the predicted transform mode is the combination of the
>     two predicted transform types;
> (d) determining a candidate transform mode for the current block by
>     rate-distortion optimisation over a plurality of candidate transform
>     modes;
> (e) comparing the candidate transform mode with the predicted transform
>     mode and, when the difference in rate-distortion cost between them
>     is within a margin expressed in units of rate, selecting the
>     predicted transform mode in place of the candidate transform mode;
> (f) generating a coded representation of the current block using the
>     selected transform mode; and
> (g) writing to the coded representation a signal indicating whether the
>     selected transform mode is the predicted transform mode, and, when
>     it is not, an indication of which of the candidate transform modes
>     other than the predicted transform mode was selected.

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
> candidate transform modes was selected is two bins selecting between the
> candidate transform modes other than the predicted transform mode, in a
> canonical order.

### Claim 5 — the prediction function

> The method of claim 1, wherein each prediction function is a multinomial
> logistic regression over the features, and wherein each prediction
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
> (c) applying a first trained prediction function to the features to
>     obtain a predicted transform type for a horizontal axis of the
>     current block, and applying a second trained prediction function to
>     the features to obtain a predicted transform type for a vertical
>     axis of the current block, and combining the two to obtain a
>     predicted transform mode, in which a predicted DCT-II on either axis
>     yields a predicted transform mode of the plain DCT-II on both axes;
> (d) reading from the coded representation a signal indicating whether
>     the predicted transform mode was used;
> (e) when the signal indicates that the predicted transform mode was
>     used, transforming the current block using the predicted transform
>     mode; and
> (f) when the signal indicates that the predicted transform mode was not
>     used, reading an indication of which of the candidate transform modes
>     other than the predicted transform mode was used, and transforming
>     the current block using that transform mode.

### Claim 7 — training

> A method of training a prediction function for use in the encoder or
> decoder of claim 1 or claim 6, the method comprising:
>
> (a) encoding a training set of pictures with a reference encoder, the
>     reference encoder performing rate-distortion optimisation over the
>     candidate transform modes;
> (b) for each block of the training set, recording the geometric features
>     of the block's causal neighbourhood as reconstructed by the
>     reference encoder, the transform type the reference encoder selected
>     for the block on the horizontal axis, and the transform type the
>     reference encoder selected for the block on the vertical axis; and
> (c) fitting the prediction function to the recorded features and the
>     recorded transform type of the horizontal axis, and fitting a
>     second prediction function to the recorded features and the recorded
>     transform type of the vertical axis, the two prediction functions
>     being fitted separately.

### Claim 8 — the margin

> The method of claim 1, wherein the margin is a function of a measured
> rate-distortion cost difference converted into a rate difference through
> the rate-distortion Lagrange multiplier, so that the margin is
> expressed in the same units as the signalling it avoids.

### Distinctions, claim by claim

| Claim | Closest reference | Why the reference does not anticipate the claim |
|---|---|---|
| 1 | EP3977732B1 (InterDigital) | The reference derives the transform from the block's width and height by table lookup. Claim 1 derives it from a gradient orientation distribution over reconstructed samples, predicts the two axes separately and composes them, and adds the rate-distortion margin of element (e), which the reference has no counterpart for. |
| 1 | US11363302B2 (MediaTek) | The reference's neural network takes the current block's own reconstruction as input, which the decoder does not have at the point the transform is needed. Claim 1's features come from a *causal* neighbourhood that excludes the current block, the two axes are predicted by separately trained functions, and the reference has no margin. |
| 2 | any | No reference found forms a gradient orientation distribution over a causal window. |
| 3 | US12143605 | That reference uses side information associated with the prediction, not a spatial window of reconstructed samples. |
| 4 | any | No reference found codes a correction relative to a predicted value in a canonical order. The correction here selects among the four modes the prediction ruled out, in two bins. |
| 5 | US11363302B2 | The reference trains a network, but not on the encoder's own rate-distortion decisions, and its output is not a transform type chosen by a margin. |
| 5, 7 | none found | No reference found trains two prediction functions separately, one per axis, and composes them into the mode. The measurement behind the split is in §1.4: the joint mode is barely predictable while each axis is not. |
| 6 | EP3977732B1 | The reference's decoder derives the transform from syntax alone. Claim 6's decoder derives it from geometry, composes two per-axis predictions, and resolves a mismatch against a canonical ordering. |
| 7 | US11363302B2 | The reference does not disclose collecting the encoder's own rate-distortion decisions over a causal neighbourhood, nor fitting the two axes separately. |
| 8 | none found | No reference found expresses the signalling-versus-distortion trade as a rate-denominated margin. |

---

## 4. Prior art — the novelty risk, stated plainly

Three facts a counsel will want to hear from the inventor rather than
discover later.

**The concept of implicit transform selection is not new.** Four families
of reference do it. A claim whose novelty rests on "the transform mode is
derived rather than signalled" would not survive §2. The claims above are
drafted to rest on the geometric features, the causal neighbourhood, the
per-axis prediction and composition, and the rate-distortion margin
instead.

**The measured result is a small win, and the closest prior art reports a
loss.** InterDigital's EP3977732B1 reports +0.34% overall for its
implicit transform design against an MTS-disabled anchor: their approach
did not pay. The present disclosure's −0.239% at a margin of 50 is a
better result by a different mechanism, and it is still far from a
decisive win. A filing that claimed a large coding gain would be
contradicted by the reference implementation in this repository.

**The measured envelope is itself the package's strongest asset.** A
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

A security audit of the portal as delivered found three exploitable
defects, which were then fixed, each with a test that fails against the
unfixed code:

- **Path traversal.** A result upload's file name and a registered
  artifact's slug and version were joined onto a path unsanitised. A
  file named `../../escaped.txt` was written outside the upload
  directory, and an artifact slug could name a path anywhere on disk.
  The names are now reduced to a conservative character set and every
  destination is resolved and checked to be inside the artifact root.
- **A forged audit trail.** The address recorded in the audit log came
  from the `X-Forwarded-For` header unconditionally, and a client can
  send any value it likes. The header is now trusted only when the
  deployment says a proxy sets it, which is the default-off case.
- **No rate limit on sign-in.** A portal that hands out confidential
  material is worth guessing at. Sign-in is now limited per address,
  with a lockout.

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
