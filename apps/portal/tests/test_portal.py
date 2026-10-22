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


# ---------------------------------------------------------------- security


def test_an_upload_cannot_escape_the_upload_directory(environment):
    """A result file's name is the uploader's, and must not choose a path.

    Without the fix, a file named `../../escaped.txt` is written outside
    the upload directory.
    """
    client, settings, root = environment
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
    _approve_request(settings, "EVAL-0001")

    response = client.post(
        "/requests/EVAL-0001/results",
        data={"label": "run", "summary": "s"},
        files={"file": ("../../escaped.txt", b"payload", "text/plain")},
    )
    assert response.status_code == 200, response.text

    assert not (root / "escaped.txt").exists(), "the upload escaped the upload directory"
    assert not (settings.artifact_root.parent / "escaped.txt").exists()


def test_a_registered_artifact_cannot_escape_the_store(environment):
    """The slug and version of a registered artifact must not choose a path."""
    client, settings, root = environment
    register(client)
    _promote_to_admin(client, settings)

    payload = b"the evaluation package"
    incoming = settings.artifact_root / "incoming"
    incoming.mkdir(parents=True, exist_ok=True)
    (incoming / "normal").write_bytes(payload)

    response = client.post(
        "/admin/artifacts",
        data={
            "slug": "../../escaped",
            "title": "T",
            "description": "D",
            "version": "../../../etc",
            "requires_agreement_version": "2026-10-01",
        },
    )
    # The slug no longer names a file, so the registration is refused.
    assert response.status_code == 404, response.text
    assert not (root / "escaped-etc.tar.gz").exists()
    assert not (root.parent / "escaped-etc.tar.gz").exists()


def test_the_forwarded_header_does_not_spoof_the_audit_log(environment):
    """A client cannot choose the address the audit log records.

    The `X-Forwarded-For` header is client-controlled, so it is only
    trusted when the deployment says a proxy sets it.
    """
    client, settings, _ = environment
    register(client)
    client.post(
        "/login",
        data={"email": "lead@acme.invalid", "password": "wrong-password-here"},
        headers={"X-Forwarded-For": "203.0.113.9"},
    )

    database = Database(_settings_of(client))
    from girc_eval import audit

    with database.session() as session:
        addresses = [entry.source_ip for entry in audit.recent(session, limit=50)]

    assert "203.0.113.9" not in addresses, "the forged address was recorded"


def test_the_forwarded_header_is_trusted_when_a_proxy_is_configured(environment):
    """A deployment behind a proxy records the address the proxy reports."""
    client, settings, _ = environment
    settings.trust_forwarded_for = True
    register(client)
    client.post(
        "/login",
        data={"email": "lead@acme.invalid", "password": "wrong-password-here"},
        headers={"X-Forwarded-For": "203.0.113.9"},
    )

    database = Database(_settings_of(client))
    from girc_eval import audit

    with database.session() as session:
        addresses = [entry.source_ip for entry in audit.recent(session, limit=50)]

    assert "203.0.113.9" in addresses


def test_repeated_failed_sign_ins_are_rate_limited(environment):
    """A guessing run is stopped rather than allowed to continue."""
    client, settings, _ = environment
    register(client)
    for _ in range(settings.max_login_attempts):
        response = client.post(
            "/login",
            data={"email": "lead@acme.invalid", "password": "wrong-password-here"},
        )
        assert response.status_code == 401, response.text
    response = client.post(
        "/login",
        data={"email": "lead@acme.invalid", "password": "wrong-password-here"},
    )
    assert response.status_code == 429, "the address was not locked out"


def test_safe_filename_rejects_a_directory_component():
    from girc_eval.security import safe_filename

    assert "/" not in safe_filename("../../etc/passwd")
    assert ".." not in safe_filename("../../etc/passwd")
    assert safe_filename("../../etc/passwd") == "passwd"
    assert safe_filename("") == "file"
    assert safe_filename("..") == "file"
    assert safe_filename("report-2026.pdf") == "report-2026.pdf"


def test_resolve_within_refuses_a_path_outside_the_root():
    from girc_eval.security import resolve_within

    root = Path(tempfile.mkdtemp(prefix="girc-root-"))
    try:
        assert resolve_within(root, "artifacts", "a.tar.gz")
        with pytest.raises(ValueError):
            resolve_within(root, "..", "outside.txt")
    finally:
        shutil.rmtree(root, ignore_errors=True)


# ---------------------------------------------------------------------------
# The sign-in rate limiter
# ---------------------------------------------------------------------------


def test_the_memory_limiter_locks_an_address_out():
    from girc_eval.ratelimit import MemoryLimiter

    limiter = MemoryLimiter(max_attempts=3, lockout_seconds=60)
    assert limiter.allowed("198.51.100.7"), "a fresh address may sign in"
    for _ in range(3):
        limiter.record_failure("198.51.100.7")
    assert not limiter.allowed("198.51.100.7"), "the address was not locked out"
    assert limiter.allowed("198.51.100.8"), "another address is unaffected"


def test_a_successful_sign_in_clears_the_failures():
    from girc_eval.ratelimit import MemoryLimiter

    limiter = MemoryLimiter(max_attempts=2, lockout_seconds=60)
    limiter.record_failure("198.51.100.7")
    limiter.clear_failures("198.51.100.7")
    assert limiter.allowed("198.51.100.7")


def test_the_redis_limiter_is_chosen_when_it_is_configured():
    from girc_eval.config import Settings
    from girc_eval.ratelimit import RedisLimiter, limiter_from_settings

    settings = Settings(redis_url="https://example.invalid", redis_token="token")
    assert isinstance(limiter_from_settings(settings), RedisLimiter)
    # With no endpoint configured, the in-memory limiter is used.
    assert not isinstance(limiter_from_settings(Settings()), RedisLimiter)


def test_the_redis_limiter_speaks_the_upstash_command_shape():
    """The limiter must send the commands Upstash's REST API expects.

    The API takes a JSON array of command arrays, and answers with
    ``{"result": ...}``. A limiter that sent the wrong shape would fail
    open, which is the worst possible failure for a rate limiter.
    """
    from girc_eval.ratelimit import RedisLimiter

    sent: list[list[str]] = []

    class FakeResponse:
        def __init__(self, result):
            self._result = result

        def raise_for_status(self):
            return None

        def json(self):
            return {"result": self._result}

    class FakeClient:
        def __init__(self):
            self.headers: dict[str, str] = {}

        def post(self, url, headers=None, json=None):
            sent.append(json)
            return FakeResponse(1)

    limiter = RedisLimiter(
        url="https://example.invalid",
        token="secret-token",
        max_attempts=10,
        client=FakeClient(),
    )
    assert limiter.allowed("198.51.100.7")
    assert sent, "the limiter sent nothing"
    assert sent[0][0] == "ZREMRANGEBYSCORE", sent
    assert sent[0][1].startswith("girc-eval:login:"), sent
    assert "198.51.100.7" not in str(sent), "the address must not be in the key"

    limiter.record_failure("198.51.100.7")
    commands = [entry[0] for entry in sent]
    assert "ZADD" in commands, commands
    assert "EXPIRE" in commands, commands
