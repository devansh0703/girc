"""The confidential evaluation agreement.

The agreement's text is versioned. When an organisation accepts it, the
version and the digest of the text are recorded, so a later edit to the
document cannot retroactively change what was agreed to.

The text is written to be a genuine confidential evaluation agreement: it
covers what the evaluator receives, what the recipient may do with it, how
the results may be used, and what happens to them at the end of the
evaluation.
"""

from __future__ import annotations

import hashlib

#: The current version of the agreement.
CURRENT_VERSION = "2026-10-01"

#: The agreement's text.
AGREEMENT_TEXT = """CONFIDENTIAL EVALUATION AGREEMENT

This Confidential Evaluation Agreement (the "Agreement") is entered into
between the organisation identified below (the "Recipient") and the
evaluator that operates this portal (the "Evaluator").

1. PURPOSE. The Evaluator grants the Recipient a non-exclusive,
non-transferable, revocable right to evaluate the Evaluator's proprietary
geometry-inference codec research package (the "Evaluation Package") against
the Recipient's own codec implementation, for the evaluation term stated in
the Recipient's evaluation request (the "Evaluation").

2. EVALUATION PACKAGE. The Evaluation Package consists of the reference
implementation, the trained inference model, the benchmark harness, the
reproducibility manifest, and any accompanying documentation made available
to the Recipient through this portal. The Evaluator may copy the Evaluation
Package only to the extent necessary to conduct the Evaluation, and must
retain the confidentiality markings on it.

3. CONFIDENTIALITY. The Evaluation Package and all results, measurements,
benchmark data, model weights and documentation derived from it are the
Evaluator's Confidential Information. The Recipient shall:
  (a) use the Confidential Information solely for the Evaluation;
  (b) not disclose the Confidential Information to any third party, including
      affiliates, contractors and advisers, without the Evaluator's prior
      written consent;
  (c) protect the Confidential Information with at least the same degree of
      care it applies to its own confidential information, and in no event
      less than reasonable care;
  (d) not reverse engineer, decompile, disassemble or otherwise attempt to
      derive the underlying ideas of the Confidential Information, except to
      the limited extent that such restriction is prohibited by applicable
      law, in which case the Recipient shall notify the Evaluator in writing
      before acting.

4. RESULTS. The Recipient grants the Evaluator a perpetual, irrevocable,
worldwide, royalty-free licence to use, reproduce, publish and distribute the
results of the Evaluation, including rate-distortion measurements, decoder
complexity measurements and any derived findings, in each case without
restriction and without any obligation to disclose the Recipient's
Confidential Information. The Recipient may not publish the Evaluation
Package itself or any part of it.

5. NO LICENCE; NO WARRANTY. The Evaluation Package is provided for evaluation
only. No licence or other right in the Evaluator's intellectual property is
granted except as expressly stated in this Agreement. The Evaluation
Package is provided "as is", without warranty of any kind, and the
Evaluator's liability under this Agreement is limited to the extent
permitted by applicable law.

6. TERM AND RETURN. This Agreement runs from the date of acceptance until
the end of the Evaluation, unless ended earlier by the Evaluator. On the end
of the Evaluation the Recipient shall cease use of the Evaluation Package and
delete all copies of it, and shall confirm deletion to the Evaluator in
writing within ten business days.

7. GOVERNING LAW AND REMEDIES. This Agreement is governed by the laws
applicable at the Evaluator's principal place of business, without regard
to its conflict-of-laws rules. The Recipient acknowledges that unauthorised
use or disclosure of the Evaluation Package would cause the Evaluator
irreparable harm for which damages alone would be an inadequate remedy.

8. ENTIRE AGREEMENT. This Agreement is the entire agreement between the
parties on its subject matter and supersedes any earlier discussion,
proposal or agreement, written or oral.

ACCEPTED BY THE RECIPIENT

Organisation: ______________________________________
Name: ____________________________________________
Title: _____________________________________________
Date: _____________________________________________
"""


def agreement_digest(text: str = AGREEMENT_TEXT) -> str:
    """The SHA-256 digest of the agreement's text."""
    return hashlib.sha256(text.encode("utf-8")).hexdigest()
