"""Build the claim chart for the prior-art analysis.

Anticipation is judged on the claims, not the description, so the
analysis needs the claim text of every reference. The claim text comes
from `extract_claims.py` (for patents whose PDF has a text layer) and
`ocr_claims.py` (for the ones that are images). Both write a
`.claims.txt` file next to the PDF, and this reads those.

The chart is a reading of the claims, not a pattern match. Each
element records the claim language that supports it, so a reader can
check the conclusion against the claim text itself.

    python3 tools/build_claim_charts.py
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CLAIMS = ROOT / "research" / "patent-package" / "claims"

#: The elements of the disclosure.
#:
#: E1  the transform is derived implicitly, without explicit signalling
#: E2  the derivation reads the geometry of a neighbourhood of
#:     reconstructed samples
#: E3  the horizontal and the vertical axis are predicted separately
#: E4  the encoder may take a less optimal transform, at a
#:     rate-distortion cost, to avoid signalling the correction
#: E5  the prediction comes from a trained or learned model
#: E6  a correction is signalled relative to a prediction
ELEMENTS = {
    "E1_implicit": "the transform is derived implicitly, without explicit signalling",
    "E2_geometry": "the derivation reads the geometry of a neighbourhood of reconstructed samples",
    "E3_per_axis": "the horizontal and the vertical axis are predicted separately",
    "E4_margin": (
        "the encoder may take a less optimal transform, at a rate-distortion cost, "
        "to avoid signalling the correction"
    ),
    "E5_learned": "the prediction comes from a trained or learned model",
    "E6_correction": "a correction is signalled relative to a prediction",
}

#: Every reference, with what its claims actually say.
#:
#: The `elements` are the ones the claims disclose, and the `evidence`
#: is the claim language that supports them. An element that is not
#: listed is not disclosed by the claims.
REFERENCES = {
    "US11297348B2": {
        "title": "Implicit transform settings for coding a block of pixels",
        "assignee": "MediaTek",
        "source": "pdf",
        "elements": {
            "E1_implicit": (
                "Claim 1: 'deriving a transform setting based on a block processing setting "
                "for the block of pixels' and 'wherein deriving the transform setting comprises "
                "selecting a transform mode from a plurality of transform modes based on the "
                "block processing setting'. The transform mode is derived, not signalled."
            ),
            "E3_per_axis": (
                "Claim 1 and claim 10: 'the position is used to determine a transform type "
                "from the set of transform types for a horizontal transform, and when the "
                "horizontal splitting orientation is used, the position is used to determine "
                "the transform type for a vertical transform'. The horizontal and the vertical "
                "transform are chosen separately, but from the block's partitioning "
                "orientation and the transformed sub-block's position, not from the "
                "neighbourhood's geometry."
            ),
        },
        "notes": (
            "The claim is about a sub-block transform: the transform type is a function of the "
            "SBT partitioning, the sub-block's position and the block's size. There is no "
            "geometry of reconstructed samples, no learned model, and no rate-distortion "
            "margin in the claims."
        ),
    },
    "EP3977732B1": {
        "title": "Implicit multiple transform selection",
        "assignee": "InterDigital",
        "source": "pdf",
        "elements": {
            "E1_implicit": (
                "Claim 1: 'obtaining, for at least one video block, syntax information "
                "indicative of use of an implicit multiple transform selection mode' and "
                "'wherein the at least one transform is selected based on the first indication "
                "and/or the second indication'. The transform is selected without a per-block "
                "signalling element."
            ),
        },
        "notes": (
            "The transform is selected from the low frequency non-separable transform mode and "
            "the matrix-based intra prediction mode, which are syntax elements of the "
            "bitstream, not a reading of the reconstructed neighbourhood. The description "
            "reports the same idea being tried in VTM: the anchor is VTM-5.0 with MTS "
            "disabled and the test is '--MTS=0 --MTSImplicit=1'."
        ),
    },
    "US12219138B2": {
        "title": "Method and apparatus for implicit transform selection",
        "assignee": "InterDigital",
        "source": "ocr",
        "elements": {
            "E1_implicit": (
                "Claim 1: 'determining, for a current transform block of a video, whether to "
                "apply a transform coefficient zero-out based on an implicit selected transform "
                "of a multiple transform selection process for a primary transform; and ... "
                "wherein the implicit selected transform of the multiple transform selection "
                "process is in an absence of one or more transform indices'."
            ),
            "E3_per_axis": (
                "Claim 1: 'wherein one or more transform matrices applied to the current "
                "transform block comprise at least one of a horizontal transform matrix or a "
                "vertical transform matrix that is based on a set of representative "
                "coefficients of the current transform block'. The horizontal and the vertical "
                "matrix are addressed separately, but they are a function of the block's own "
                "coefficients, not of a prediction."
            ),
        },
        "notes": (
            "The claim is about a transform coefficient zero-out. The transform itself is the "
            "implicitly selected one, and the claim adds the zero-out as the novel matter."
        ),
    },
    "US11134275B2": {
        "title": "Implicit transform type signalling",
        "assignee": "Qualcomm",
        "source": "pdf",
        "elements": {
            "E1_implicit": (
                "Claim 1: 'in response to determining that the filtering process is applied on "
                "the current block, performing, based on the filtering process, a primary "
                "transform for a transform block that is partitioned from the current block'. "
                "The decoder performs the transform; the transform mode is not signalled per "
                "block."
            ),
            "E2_geometry": (
                "Claim 1: 'the filtering process adjusts inter prediction samples of the "
                "current block based on neighboring reconstructed samples of the current "
                "block'. The neighbouring reconstructed samples are read, but they are used to "
                "filter the prediction, and the transform is chosen from the filtering mode "
                "and the block's size, not from the samples' geometry."
            ),
            "E3_per_axis": (
                "Claim 3: 'the primary transform is a type-7 discrete sine transform (DST-7) "
                "mode for a horizontal transform that is applied to the transform block "
                "responsive to a width of the current block being smaller than or equal to a "
                "threshold value, the primary transform is the DST-7 mode for a vertical "
                "transform that is applied to the transform block responsive to a height of "
                "the current block being smaller than or equal to the threshold value'. The "
                "horizontal and the vertical transforms are chosen separately, but each is a "
                "function of the block's width or height, not of a prediction."
            ),
        },
        "notes": (
            "The transform is a function of the filtering mode, the CIIP or PDPC mode, the "
            "block's width and height, and whether the neighbours are intra coded. The "
            "neighbouring blocks' coding mode is a form of neighbourhood context, but it is "
            "their prediction mode, not the geometry of their reconstructed samples."
        ),
    },
    "US20230086769A1": {
        "title": "Implicit transform type signalling",
        "assignee": "Qualcomm",
        "source": "ocr",
        "elements": {
            "E1_implicit": (
                "Claim 1: 'determining whether an implicit MTS is enabled for a current block "
                "responsive to ... the second HLS element indicating that the explicit MTS for "
                "intra coding is disabled, and ... a prediction mode of the current block is an "
                "intra prediction mode'. Claim 6: 'wherein no transform type information is "
                "signaled by the implicit MTS'."
            ),
        },
        "notes": (
            "The claim covers the enabling of an implicit MTS by high-level syntax and the "
            "block's prediction mode. The transform itself is not derived from the "
            "reconstructed neighbourhood."
        ),
    },
    "US11785254B2": {
        "title": "Implicit mode dependent primary transforms",
        "assignee": "Apple and Qualcomm",
        "source": "ocr",
        "elements": {
            "E1_implicit": (
                "Claim 1: 'selecting a subset of hybrid transform kernels from among the set "
                "of hybrid transform kernels; and decoding the video data based on the subset "
                "of hybrid transform kernels that is selected'. Claim 2: 'wherein the subset of "
                "hybrid transform kernels is selected implicitly, based on coded information "
                "that is available to both encoder and decoder'."
            ),
            "E3_per_axis": (
                "Claim 1: 'wherein the set of hybrid transform kernels comprise a plurality of "
                "pairs of transform types, each of the plurality of pairs including a vertical "
                "transform type and a horizontal transform type'. The per-axis pair structure "
                "is claimed explicitly."
            ),
        },
        "notes": (
            "This is the closest art to the per-axis element: it claims a set of transform "
            "pairs, each with a vertical and a horizontal transform type, selected implicitly. "
            "Claim 3 narrows the selection to 'both of an intra prediction mode and a block "
            "size'. The transforms are AV1's line graph transform pairs, and the selection "
            "reads the intra prediction mode and the block size, not the geometry of the "
            "reconstructed neighbourhood."
        ),
    },
    "HK40058354A": {
        "title": "Implicit transform selection in video coding",
        "assignee": "Qualcomm",
        "source": "chinese",
        "elements": {
            "E1_implicit": (
                "Claim 1: inferring the transform type for the current transform block from a "
                "plurality of transform types comprising one or more discrete cosine transforms "
                "(DCTs) and one or more discrete sine transforms (DSTs), where the inference "
                "is of the current transform block's type rather than a signalled index."
            ),
            "E3_per_axis": (
                "Claim 10: 'selecting the transform type comprises: selecting the transform "
                "type for horizontal use and selecting the transform type for vertical use'. "
                "The horizontal and the vertical transforms are selected separately."
            ),
        },
        "notes": (
            "The claims are in Chinese and were read from the filed text. The inference is of "
            "the transform type from the block's size, its ISP partitioning and the width and "
            "height thresholds, and it is enabled by the MTS being on (claim 13). There is no "
            "geometry of reconstructed samples and no learned model."
        ),
    },
    "EP2991352A1": {
        "title": "Methods and apparatus for transform selection in video encoding and decoding",
        "assignee": "Thomson Licensing",
        "source": "epo",
        "elements": {
            "E1_implicit": (
                "Claim 1: 'a video encoder for encoding at least a block in a picture by "
                "selecting a transform to apply to a residue of the block from a set of two or "
                "more transforms, wherein the transform is selected responsive to an inter "
                "prediction mode used to predict at least one reference for the block'. The "
                "transform is selected by the encoder and derived by the decoder, not "
                "signalled per block."
            ),
            "E2_geometry": (
                "Claim 2: 'wherein the set of two or more transforms is derived from "
                "reconstructed video data from one or more previous pictures'. The set comes "
                "from reconstructed data, and claim 8 says the same for the decoder."
            ),
            "E3_per_axis": (
                "Claim 4: 'wherein the transforms in the set are any of separable or "
                "non-separable, and horizontal and vertical directions of the transforms are "
                "capable of being different when the transforms are separable'. The horizontal "
                "and the vertical transform are separately identifiable."
            ),
            "E5_learned": (
                "Claim 3: 'wherein the set of transforms is determined by at least one of: "
                "(i) using training data, and (ii) choosing a subset of a family of "
                "transforms'. The transform set is determined by training data."
            ),
        },
        "notes": (
            "Priority 2009-01-27, so this is the oldest reference in the set. It claims the "
            "transform set derived from reconstructed video data, determined using training "
            "data, with separable horizontal and vertical transforms that may differ, and "
            "with the transform signalled only above a threshold block size (claim 5) and a "
            "flag embedded in the coefficients below it (claim 6). That is the closest art "
            "to the whole of the disclosure, and it is twenty years old."
        ),
    },
    "US9357221B2": {
        "title": "Methods and apparatus for adaptive transform selection for video encoding and decoding",
        "assignee": "Thomson Licensing",
        "source": "pdf",
        "elements": {
            "E1_implicit": (
                "Claim 1: 'the video encoder configured to encode at least a block using a "
                "transform selected from a plurality of transforms and to encode at least a "
                "second block using a second transform selected from an adaptively updated "
                "plurality of transforms'. The transform comes from a plurality the decoder "
                "maintains."
            ),
            "E2_geometry": (
                "Claim 1: 'wherein the transformer is configured to determine the adaptively "
                "updated plurality of transforms by adaptively updating the plurality of "
                "transforms, after one or more pictures are encoded, based on residue "
                "information from the encoded one or more pictures'. The plurality is "
                "derived from the residual of already-coded pictures."
            ),
            "E5_learned": (
                "Claim 23: 'wherein the plurality of transforms are adaptively updated to "
                "obtain a trained plurality of transforms, and a selection of the subset is "
                "based on the trained plurality of transforms'."
            ),
            "E6_correction": (
                "Claim 20: 'wherein the video encoder is further configured to transmit the "
                "selected transform to a corresponding decoder when the selected transform is "
                "not present at the corresponding decoder, to identify the selected transform "
                "... when the selected transform is already present at the corresponding "
                "decoder'. The encoder signals the transform only when the decoder's "
                "plurality does not already contain it, and otherwise the decoder regenerates "
                "it. This is the correction-relative-to-a-prediction arrangement."
            ),
        },
        "notes": (
            "The same family as EP2991352A1, in its granted United States form. Claim 20 is "
            "the closest art to the correction element: the decoder holds a plurality of "
            "transforms, and the encoder transmits the selected one only when the decoder's "
            "plurality does not already hold it."
        ),
    },
    "US20240040151A1": {
        "title": "Coefficient-based transform and mode signaling",
        "assignee": "Apple",
        "source": "ocr",
        "elements": {
            "E1_implicit": (
                "Claim 7: 'when the determined pattern matches a pre-defined pattern, "
                "determining whether a transform type used in coding matches a default "
                "transform type associated with the pre-defined pattern; and when the "
                "determined pattern matches the pre-determined pattern, the transmitted coded "
                "data of the transform unit omits an identification of a transform type "
                "applied during the transforming'. The transform type is inferred at the "
                "decoder and not signalled."
            ),
            "E2_geometry": (
                "The abstract: 'based on the spatial locations of decoded coefficients, the "
                "spatial patterns of coefficients, or the correlation with the coefficients "
                "in neighboring blocks, various syntax elements such as the transform type "
                "and related flags/indices ... may be disabled or constrained'. The "
                "correlation with the neighbouring blocks' coefficients is the neighbourhood "
                "context."
            ),
            "E6_correction": (
                "Claim 2: 'comparing a transform type applied during the transforming to a "
                "default transform type defined by the coding protocol and, when a transform "
                "type applied during the transforming and the default transform type defined "
                "by the coding protocol are the same, omitting an identifier of the transform "
                "type from the coded data'. The signalling is relative to a default, and it "
                "is omitted when the default matches."
            ),
        },
        "notes": (
            "Priority 2022-07-28, so this is the most recent reference. It derives the "
            "transform type, and a range of other syntax elements, from the pattern of the "
            "decoded coefficients and their correlation with the neighbouring blocks, and it "
            "omits the signalling when the inferred value matches a default. The inference "
            "is from the coefficients rather than from the reconstructed pixels' geometry, "
            "and the omission is a match against a default rather than a rate-distortion "
            "trade."
        ),
    },
    "US11363302B2": {
        "title": "Neural post-filter for video coding",
        "assignee": "MediaTek",
        "source": "pdf",
        "elements": {},
        "notes": (
            "Not transform-selection art. The claims are about a neural network post-filter "
            "that generates a filtered reconstruction from a luma and a chroma component. An "
            "earlier table in this project listed it as transform art, and that was wrong; "
            "the relevant MediaTek reference is US11297348B2."
        ),
    },
}


def main() -> int:
    charts = {}
    for number, reference in REFERENCES.items():
        claims_path = CLAIMS / f"{number}.claims.txt"
        claims = ""
        if claims_path.exists():
            claims = claims_path.read_text(errors="replace")
        else:
            print(f"{number}: no claims text, skipped", file=sys.stderr)
            continue

        elements = {}
        for element, evidence in reference.get("elements", {}).items():
            elements[element] = {
                "description": ELEMENTS[element],
                "present": True,
                "evidence": evidence,
            }
        for element in ELEMENTS:
            if element not in elements:
                elements[element] = {
                    "description": ELEMENTS[element],
                    "present": False,
                    "evidence": "",
                }

        charts[number] = {
            "title": reference["title"],
            "assignee": reference["assignee"],
            "source": reference["source"],
            "claim_characters": len(claims),
            "elements_present": sorted(elements),
            "elements": elements,
            "notes": reference.get("notes", ""),
            "claims": claims[:10000],
        }
        print(f"{number}: {len(claims)} characters; elements present: {sorted(elements)}")

    destination = ROOT / "research" / "patent-package" / "claim_charts.json"
    destination.write_text(json.dumps(charts, indent=2))
    print(f"wrote {destination}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
