# The prior art search

This is the search that was outstanding. It is a public-record search,
not a legal opinion, and it does not substitute for counsel. What it
does is establish which parts of the disclosure are already disclosed by
published claims, so that the claim that is actually filed is the part
that is not.

## What was searched

Anticipation is judged on the claims, not the description, so the search
is a claim search. For every reference below, the claims were extracted
from the publication's own PDF and read:

- `tools/extract_claims.py` reads the claims of a PDF with a text layer.
  A patent page is two columns, so the words are read with their
  positions and only the claims column is kept.
- `tools/ocr_claims.py` reads the claims of a PDF that is an image.
  Some grant records are published as scans, and `tesseract` reads them.
- `tools/build_claim_charts.py` records the reading, element by element,
  with the claim language that supports each one, and writes
  `claim_charts.json`.

The extracted claim text is in `claims/`, one file per reference, and is
the evidence for everything below. The PDFs themselves are not in the
repository; they are fetched from the publication servers.

The search ran against Google Patents, the EPO's publication server, and
the Xiph.Org sequence archive. The candidate set came from keyword
searches on: implicit transform selection, derived transform type, per-axis
transform selection, neighbour-derived transform type, decoder-side
transform derivation, learned transform prediction, rate-distortion
constrained mode selection, and signalling a correction relative to a
prediction.

## The elements

The disclosure is broken into six elements, and the question for each is
whether the claims of any reference disclose it.

| | Element |
|---|---|
| E1 | the transform is derived implicitly, without explicit signalling |
| E2 | the derivation reads the geometry of a neighbourhood of reconstructed samples |
| E3 | the transform is selected per axis, so that the horizontal and the vertical axis can differ |
| E4 | the encoder may take a less optimal transform, at a rate-distortion cost, to avoid signalling the correction |
| E5 | the prediction comes from a trained or learned model |
| E6 | a correction is signalled relative to a prediction |

## The result

| Reference | Priority | Assignee | E1 | E2 | E3 | E4 | E5 | E6 |
|---|---|---|:-:|:-:|:-:|:-:|:-:|:-:|
| EP2991352A1 | 2009-01-27 | Thomson Licensing | ● | ● | ● | | ● | |
| US9357221B2 | 2009-07-23 | Thomson Licensing | ● | ● | | | ● | ● |
| US11297348B2 | 2018-04-13 | MediaTek | ● | | ● | | | |
| EP3977732B1 | 2019-05-31 | InterDigital | ● | | | | | |
| US11134275B2 | 2019-06-04 | Tencent America | ● | ● | ● | | | |
| HK40058354A | 2020-03-12 | Qualcomm | ● | | ● | | | |
| US11785254B2 | 2020-05-29 | Tencent America | ● | | ● | | | |
| US12219138B2 | 2020-06-08 | ByteDance | ● | | ● | | | |
| US20230086769A1 | 2021-09-01 | Tencent America | ● | | | | | |
| US20240040151A1 | 2022-07-28 | Apple | ● | | | | | ● |

**E1, E2, E3, E5 and E6 are all disclosed. E4 alone is not.**

That is the finding, and it is a negative one for the broad claims. It
also says where the filing should go.

## Why each element is disclosed

**E1, implicit derivation.** Every reference in the set claims it in
some form. EP3977732B1 claim 1: *"obtaining, for at least one video block, syntax information indicative of use of an implicit multiple transform selection mode ... wherein the at least one transform is selected based on the first indication and/or the second indication."*
US20230086769A1 claim 6: *"wherein no transform type information is signaled by the implicit MTS."* VVC itself contains an implicit MTS
mode, so a claim to deriving the transform type without signalling it
reads on the standard.

**E2, the neighbourhood's geometry.** EP2991352A1 claim 2: *"wherein the set of two or more transforms is derived from reconstructed video data from one or more previous pictures."* US9357221B2 claim 1 derives
the transform plurality *"based on residue information from the encoded one or more pictures."* US11134275B2 claim 1 reads the *"neighboring reconstructed samples of the current block."*

US20240040151A1 goes furthest, but its claims do not reach it. The
description says that the transform type may be *"disabled or constrained"* based on *"the spatial locations of decoded coefficients, the spatial patterns of coefficients, or the correlation with the coefficients in neighboring blocks"*. Both passages are from the description and not from a claim, so they are marked as such and are not checked against the claim text. Its claims do not: claim 7 reads the
block's own coefficient pattern and nothing beyond it. Anticipation is
judged on the claims, so the element is not recorded as disclosed by
that reference, but a claim drafted on the description's language would
be at risk from it.

The distinction that survives is the source of the geometry. The
references read reconstructed *pixels* or *coefficients*; the disclosure
reads the *orientation structure* of a causal window, as a structure
tensor over the reconstructed neighbourhood, and combines it with the
already-decoded neighbours' transform modes.

**E3, the per-axis prediction.** US11785254B2 claim 1: *"the set of hybrid transform kernels comprise a plurality of pairs of transform types, each of the plurality of pairs including a vertical transform type and a horizontal transform type."* That is the per-axis pair
structure, claimed explicitly, and it is AV1's line graph transform
pairs. EP2991352A1 claim 4: *"horizontal and vertical directions of the transforms are capable of being different when the transforms are separable."* US11297348B2 claim 10, HK40058354A claim 10 and
US11134275B2 claim 3 each pick the horizontal and the vertical
transform separately.

This is the most crowded element, and the honest reading is that a
claim to "predict the horizontal and the vertical transform separately"
is not available. What may be available is the specific prediction: a
*learned* model over the joint mode, its features, and the RD margin
applied to the predicted mode.

The implementation predicts the joint mode with one model rather than
two per-axis models composed together, and that was a measured
decision rather than a preference. Each axis on its own was well above
the majority class, about 54.6% against 49.8%, and the composed mode
came out *below* it, at 48.3%. The five modes are not a product space:
the plain DCT-II appears on an axis only in the mode where both axes
are the plain DCT-II, and the four multiple transform selection modes
are correlated. Composing two independent per-axis models cannot
recover the joint distribution, and the measurement showed it losing.
One model over the five modes is both simpler and better.

**E5, a learned model.** EP2991352A1 claim 3: *"wherein the set of transforms is determined by at least one of: (i) using training data, and (ii) choosing a subset of a family of transforms."* US9357221B2
claim 23: *"the plurality of ... nsforms are adaptively updated to obtain
a trained plurality of ... transforms, and wherein a selection of the
subset is based on ... the plurality of transforms and the trained
plurality of".* The ellipses mark the words the optical character
recognition dropped at the edge of the column.

A claim to "use a trained model to pick the transform" is not
available. What may be available is the model's *inputs*: the causal
geometry window and the neighbour transform map, as a specific feature
set.

**E6, a correction relative to a prediction.** US9357221B2 claim 20 is
the closest art: *"the video encoder is ... ther configured to transmit
the selected transform to a ... rresponding decoder when the selected
transform is not ... 2sent at the corresponding decoder, to identify the
selected ... nsform to the corresponding decoder when the selected ...
nsform is already present at the corresponding decoder".* The decoder holds a plurality of
transforms and the encoder signals only when the decoder's plurality
does not already contain the choice. US20240040151A1 claim 2 is the
same shape against a default: *"comparing a transform type applied
during the transforming to a default transform type defined by the
coding protocol and, when a transform type applied during the
transforming and the default transform type defined by the coding
protocol are the same, omitting an identifier of the transform type
from the coded data transmitted to the channel".*

The distinction that survives is the *reason* the encoder accepts the
prediction. In both references the encoder takes the default because
the default is free. In the disclosure the encoder takes the predicted
mode when it is within a rate-distortion margin of the best mode,
which is a different decision rule and a different technical effect.

**E4, the rate-distortion margin.** No reference in the set claims it.
No claim in any of the ten reads a threshold, a margin, a sub-optimal
choice, or a trade of distortion for signalling. This is the element
with no prior art, and it is the one the implementation was built
around: the encoder accepts a slightly worse transform to avoid
signalling the correction, and the margin is the parameter that
controls the trade.

## The corrections to the earlier table

The earlier table in this project had errors, and they are corrected
here.

- **US11363302B2 is not transform art.** It is *"Neural post-filter for video coding"*, and its claims are about a neural network that
  generates a filtered reconstruction from a luma and a chroma
  component. It is not a reference for the transform elements. The
  relevant MediaTek reference is US11297348B2, which was in the set
  already and was mislabelled.
- **Several titles and assignees were wrong.** US11785254B2 is
  *"Implicit mode dependent primary transforms"*, and its assignee is
  Tencent America, not Qualcomm. US11134275B2 is *"Method and apparatus for performing primary transform based on filtering of blocks"*, also
  Tencent America, and not Qualcomm's. US11297348B2 is *"High-level syntax control on implicit transform"*'s sibling, *"Implicit transform settings for coding a block of pixels"*, and US20230086769A1
  is *"High-level syntax control on implicit transform"*. The two are
  different applications with the same family, and the earlier table
  had them the wrong way round. HK40058354A is *"Implicit transform selection in video coding"*, the Hong Kong national phase of the
  application that is US11134275B2 in the United States, and its
  claims are in Chinese. US12219138B2 is ByteDance's *"Simplified transform coding tools"*, not InterDigital's.
- **Three references were missing**, and they are the ones that matter
  most: the Thomson pair of 2009 and the Apple application of 2022.

## The risk, stated plainly

On the public record as searched:

- A broad claim to deriving the transform type without signalling it
  would be anticipated.
- A claim to predicting the horizontal and the vertical transform
  separately would be anticipated.
- A claim to using a trained model to select the transform would be
  anticipated.
- A claim to signalling the transform only when the decoder's set does
  not already contain it would be anticipated.
- A claim to reading the reconstructed neighbourhood's geometry to
  select the transform would be at risk, and Apple's application is the
  closest art to it.
- A claim to the rate-distortion-constrained acceptance of a
  sub-optimal transform to avoid the signalling would not be anticipated
  by anything found, on the public record as searched.

That last one is the only defensible core, and it is narrow. It is a
system claim and a method claim, not a codec-level one, and it would
have to be filed with the implementation as the evidence that it works
and that the trade is a real one.

## What this search did not do

- It did not search the national phase filings of the families in the
  set, so the claim sets as granted in China, Japan and Korea were not
  read. The Chinese claims of HK40058354A were read; the others were
  not.
- It did not search the non-patent literature, so the VVC and AV1
  literature on implicit transform selection is only present where a
  patent cites it.
- It did not search for the RD-margin element in a form other than the
  terms used here. The absence of E4 in the ten references is an
  absence in those ten, not a proven absence everywhere.
- It is a public-record search. Only counsel can say what is
  patentable, what is free to practise in a given jurisdiction, and
  what an accused infringer would say.

## Reproducing this

```sh
# The claim text of every reference.
python3 tools/extract_claims.py research/patent-package/claims/US11297348B2.pdf
python3 tools/ocr_claims.py research/patent-package/claims/US12219138B2.pdf 45 49

# The chart.
python3 tools/build_claim_charts.py
```

`research/patent-package/claims/pdf_urls.json` holds the address of
every publication's PDF, and `claim_charts.json` holds the chart with
the supporting claim language for each entry.
