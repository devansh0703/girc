"""GIRC Evaluate: the evaluation portal, and the hosted analysis.

Two things live behind this package. The portal is the confidential
evaluation environment: accounts, the versioned agreement, gated
artifacts, result submission, and an append-only audit log. The hosted
analysis (``girc_eval.service``) is the open half: post a sequence, get
the same figures the benchmark writes, and nothing is stored.

They share nothing at import time, on purpose. The hosted analysis runs
on a free instance that installs only what it needs, so importing the
``girc_eval`` package must not drag in the portal's database stack.
Every name in this package is imported from its own module, and only
from there; the one convenience this file keeps is the ``create_app``
factory, which imports the portal's application lazily.

The portal's flow:

1. An organisation registers, and one of its users signs the evaluation
   agreement (NDA).
2. The organisation requests an evaluation, naming the codec it wants
   to test against and the content it will test on.
3. Once the request is approved, the gated artifacts become available.
4. The organisation uploads its results.
5. A reviewer approves the results and issues an evaluation report.

Every step is written to the audit log.
"""

from __future__ import annotations


def create_app(settings=None):
    """Build the portal's FastAPI application.

    The application is created through a factory so tests can point it at
    a temporary database and a fresh settings object.
    """
    from girc_eval.app import create_app as _create_app

    return _create_app(settings)
