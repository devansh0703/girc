"""The evaluation portal's test suite.

The tests cover the whole evaluation lifecycle, and they check the things
that matter for a confidential portal: that the agreement gates the
downloads, that a download token cannot be edited to fetch a different
artifact, that the audit log records every state change, and that the
results cannot be submitted for another organisation's request.
"""

from __future__ import annotations

import shutil
import tempfile
from pathlib import Path

import pytest
from fastapi.testclient import TestClient

from girc_eval.app import create_app
from girc_eval.config import Settings
from girc_eval.db import Database


@pytest.fixture()
def environment():
    """A portal with a temporary database and artifact store."""
    root = Path(tempfile.mkdtemp(prefix="girc-eval-test-"))
    settings = Settings(
        database_url=f"sqlite:///{root / 'test.db'}",
        artifact_root=root / "artifacts",
        secret_key="test-secret-key-not-for-production",
        admin_email="admin@girc.invalid",
    )
    (settings.artifact_root / "incoming").mkdir(parents=True, exist_ok=True)

    app = create_app(settings)
    client = TestClient(app)
    yield client, settings, root
    shutil.rmtree(root, ignore_errors=True)


def register(client: TestClient, name: str = "Acme Codecs") -> None:
    """Register an organisation and its first user."""
    response = client.post(
        "/register",
        data={
            "organization_name": name,
            "email": "lead@acme.invalid",
            "password": "a-sufficiently-long-password",
            "full_name": "Lead Engineer",
        },
    )
    assert response.status_code == 200, response.text
    assert response.json()["status"] == "registered"


def sign_in(client: TestClient, email: str = "lead@acme.invalid") -> None:
    """Sign a user in."""
    response = client.post(
        "/login",
        data={"email": email, "password": "a-sufficiently-long-password"},
    )
    assert response.status_code == 200, response.text


def create_artifact(settings: Settings, slug: str = "girc-core-source") -> str:
    """Put an artifact in the incoming store and register it."""
    import hashlib

    payload = b"the evaluation package"
    incoming = settings.artifact_root / "incoming" / slug
    incoming.write_bytes(payload)
    digest = hashlib.sha256(payload).hexdigest()
    return digest


def test_the_portal_starts_and_serves_its_landing_page(environment):
    client, _, _ = environment
    response = client.get("/")
    assert response.status_code == 200
    assert "GIRC" in response.text


def test_registration_signs_the_user_in(environment):
    client, _, _ = environment
    register(client)
    # The registration response set the session cookie, so the dashboard
    # is reachable without a separate sign-in.
    response = client.get("/dashboard", follow_redirects=False)
    assert response.status_code == 200, response.text


def test_sign_in_with_the_wrong_password_is_refused(environment):
    client, _, _ = environment
    register(client)
    response = client.post(
        "/login", data={"email": "lead@acme.invalid", "password": "wrong-password-here"}
    )
    assert response.status_code == 401


def test_the_agreement_must_be_accepted_before_an_evaluation_is_requested(
    environment,
):
    client, _, _ = environment
    register(client)

    response = client.post(
        "/requests",
        data={
            "title": "Test",
            "target_codec": "VVC",
            "test_content_description": "Some content",
            "sampling_plan": "QP 22 to 37",
            "motivation": "To see whether it helps",
        },
    )
    assert response.status_code == 403, response.text


def test_an_evaluation_can_be_requested_once_the_agreement_is_accepted(environment):
    client, _, _ = environment
    register(client)

    response = client.post("/agreement/accept")
    assert response.status_code == 200, response.text
    assert response.json()["status"] == "accepted"

    response = client.post(
        "/requests",
        data={
            "title": "Transform signalling against our encoder",
            "target_codec": "VVC",
            "target_codec_version": "1.14.0",
            "test_content_description": "Some content",
            "sampling_plan": "QP 22 to 37",
            "motivation": "To see whether it helps",
        },
    )
    assert response.status_code == 200, response.text
    assert response.json()["reference"] == "EVAL-0001"


def test_the_agreement_cannot_be_accepted_twice(environment):
    client, _, _ = environment
    register(client)
    assert client.post("/agreement/accept").status_code == 200
    second = client.post("/agreement/accept")
    assert second.status_code == 200
    assert second.json()["status"] == "already-accepted"


def test_a_download_is_refused_before_an_evaluation_is_approved(environment):
    client, settings, _ = environment
    register(client)
    client.post("/agreement/accept")
    _register_artifact(settings)

    response = client.get("/artifacts/girc-core-source/download-token")
    assert response.status_code == 403, response.text


def test_an_approved_evaluation_allows_the_download(environment):
    client, settings, _ = environment
    register(client)
    _register_artifact(settings)
    client.post("/agreement/accept")
    client.post(
        "/requests",
        data={
            "title": "Test",
            "target_codec": "VVC",
            "test_content_description": "Some content",
            "sampling_plan": "QP 22 to 37",
            "motivation": "To see whether it helps",
        },
    )

    _approve_request(settings, "EVAL-0001")

    response = client.get("/artifacts/girc-core-source/download-token")
    assert response.status_code == 200, response.text
    token = response.json()["token"]
    assert token

    download = client.get(f"/download?token={token}")
    assert download.status_code == 200, download.text
    assert download.content == b"the evaluation package"


def test_a_download_token_cannot_be_edited(environment):
    client, settings, _ = environment
    register(client)
    _register_artifact(settings)
    client.post("/agreement/accept")
    client.post(
        "/requests",
        data={
            "title": "Test",
            "target_codec": "VVC",
            "test_content_description": "Some content",
            "sampling_plan": "QP 22 to 37",
            "motivation": "To see whether it helps",
        },
    )
    _approve_request(settings, "EVAL-0001")

    token = client.get("/artifacts/girc-core-source/download-token").json()["token"]
    tampered = token[:-4] + ("aaaa" if not token.endswith("aaaa") else "bbbb")
    response = client.get(f"/download?token={tampered}")
    assert response.status_code == 403, response.text


def test_results_cannot_be_submitted_for_another_organisation(environment):
    client, settings, _ = environment
    register(client, "Acme Codecs")
    client.post("/agreement/accept")
    client.post(
        "/requests",
        data={
            "title": "Test",
            "target_codec": "VVC",
            "test_content_description": "Some content",
            "sampling_plan": "QP 22 to 37",
            "motivation": "To see whether it helps",
        },
    )

    # A second organisation.
    client.post(
        "/register",
        data={
            "organization_name": "Other Codecs",
            "email": "lead@other.invalid",
            "password": "another-sufficiently-long-password",
            "full_name": "Other Lead",
        },
    )

    response = client.post(
        "/requests/EVAL-0001/results",
        data={"label": "Our results", "summary": "Nothing to see here"},
    )
    assert response.status_code == 403, response.text


def test_every_state_change_is_audited(environment):
    client, _, _ = environment
    register(client)
    client.post("/agreement/accept")
    client.post(
        "/requests",
        data={
            "title": "Test",
            "target_codec": "VVC",
            "test_content_description": "Some content",
            "sampling_plan": "QP 22 to 37",
            "motivation": "To see whether it helps",
        },
    )

    database = Database(_settings_of(client))
    from girc_eval import audit

    with database.session() as session:
        actions = [entry.action for entry in audit.recent(session, limit=50)]

    assert "organisation.registered" in actions
    assert "agreement.accepted" in actions
    assert "evaluation.requested" in actions
    assert "evaluation.requested" in actions


def test_a_failed_sign_in_is_audited(environment):
    client, _, _ = environment
    register(client)
    client.post(
        "/login", data={"email": "lead@acme.invalid", "password": "wrong-password-here"}
    )

    database = Database(_settings_of(client))
    from girc_eval import audit

    with database.session() as session:
        actions = [entry.action for entry in audit.recent(session, limit=50)]

    assert "login.failed" in actions


# ------------------------------------------------------------- helpers


def _settings_of(client: TestClient) -> Settings:
    """The settings the client was built with."""
    return client.app.state.settings


def _promote_to_admin(client: TestClient, settings: Settings) -> None:
    """Give a user the platform administrator role."""
    database = Database(settings)
    from girc_eval.models import User, UserRole

    with database.session() as session:
        user = session.query(User).filter(User.email == "lead@acme.invalid").first()
        user.role = UserRole.platform_admin


def _register_artifact(settings: Settings, slug: str = "girc-core-source") -> None:
    """Register a gated artifact, as an administrator would.

    The registration goes through the application itself, so the test
    exercises the same path an operator uses rather than writing to the
    database directly.
    """
    from girc_eval.models import User, UserRole

    database = Database(settings)
    with database.session() as session:
        user = session.query(User).filter(User.email == "lead@acme.invalid").first()
        user.role = UserRole.platform_admin

    create_artifact(settings, slug)

    app = create_app(settings)
    with TestClient(app) as admin_client:
        admin_client.cookies.clear()
        _sign_in_admin(admin_client)
        response = admin_client.post(
            "/admin/artifacts",
            data={
                "slug": slug,
                "title": "GIRC reference codec",
                "description": "The reference implementation.",
                "version": "1.0.0",
                "requires_agreement_version": "2026-10-01",
            },
        )
        assert response.status_code == 200, response.text


def _sign_in_admin(client: TestClient) -> None:
    """Sign in as the promoted administrator."""
    response = client.post(
        "/login",
        data={"email": "lead@acme.invalid", "password": "a-sufficiently-long-password"},
    )
    assert response.status_code == 200, response.text


def _approve_request(settings: Settings, reference: str) -> None:
    """Approve an evaluation request, as a reviewer would."""
    database = Database(settings)
    from girc_eval.models import EvaluationRequest, RequestStatus

    with database.session() as session:
        request = (
            session.query(EvaluationRequest)
            .filter(EvaluationRequest.reference == reference)
            .first()
        )
        request.status = RequestStatus.approved
