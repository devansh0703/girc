"""GIRC Evaluate: the confidential evaluation portal.

A codec team that wants to test the geometry-inference mechanism does it
here. The flow is:

1. An organisation registers, and one of its users signs the confidential
   evaluation agreement (NDA).
2. The organisation requests an evaluation, naming the codec it wants to
   test against and the content it will test on.
3. Once the request is approved, the gated artifacts become available: the
   reference implementation, the trained predictor model, the benchmark
   harness and the draft patent package.
4. The organisation uploads its results.
5. A reviewer approves the results and issues an evaluation report.

Every step is written to an append-only audit log.

The portal is the commercial vehicle for the patent package. It is where
the brief's "confidential evaluation agreements" and "the first customer
pays to test the method against its own codec work" actually happen.
"""

from girc_eval.config import Settings
from girc_eval.db import Database
from girc_eval.models import (
    Agreement,
    Artifact,
    AuditEntry,
    EvaluationRequest,
    Organization,
    ResultSubmission,
    User,
)

__all__ = [
    "Agreement",
    "Artifact",
    "AuditEntry",
    "Database",
    "EvaluationRequest",
    "Organization",
    "ResultSubmission",
    "Settings",
    "User",
    "create_app",
]


def create_app(settings: Settings | None = None):
    """Build the FastAPI application.

    The application is created through a factory so tests can point it at a
    temporary database and a fresh settings object.
    """
    from girc_eval.app import create_app as _create_app

    return _create_app(settings)
