"""Settings for the evaluation portal.

Every setting has a default that works for a local run, so the portal can
be started with no configuration at all. Production deployments override
them through the environment.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path


@dataclass
class Settings:
    """The portal's configuration."""

    #: The database URL. The default is a local SQLite file.
    database_url: str = "sqlite:///./var/girc-eval.db"

    #: Where gated artifacts are stored.
    artifact_root: Path = field(default_factory=lambda: Path("./var/artifacts"))

    #: The signing secret for session cookies and download tokens.
    secret_key: str = "development-only-secret-change-me"

    #: How long a session lasts, in hours.
    session_hours: int = 12

    #: How long a download link stays valid, in minutes.
    download_token_minutes: int = 30

    #: The organisation that is allowed to administer the portal.
    admin_email: str = "admin@girc.invalid"

    #: The maximum size of an uploaded result file, in bytes.
    max_upload_bytes: int = 64 * 1024 * 1024

    #: Whether a reverse proxy sets `X-Forwarded-For`.
    #:
    #: The header is client-controlled, so it is only trusted when a
    #: deployment has said a proxy in front of the portal sets it.
    #: Otherwise the socket's own address is recorded, which cannot be
    #: forged.
    trust_forwarded_for: bool = False

    #: How many failed sign-in attempts are allowed from one address
    #: before it is locked out for a while.
    #:
    #: A portal that hands out confidential material is worth guessing
    #: at, so the sign-in route is rate limited.
    max_login_attempts: int = 10

    #: How long a locked-out address is refused, in seconds.
    login_lockout_seconds: int = 900

    @classmethod
    def from_env(cls) -> "Settings":
        """Build the settings from the environment."""
        import os

        return cls(
            database_url=os.environ.get("GIRC_DATABASE_URL", cls.database_url),
            artifact_root=Path(
                os.environ.get("GIRC_ARTIFACT_ROOT", str(cls.artifact_root))
            ),
            secret_key=os.environ.get("GIRC_SECRET_KEY", cls.secret_key),
            session_hours=int(os.environ.get("GIRC_SESSION_HOURS", cls.session_hours)),
            download_token_minutes=int(
                os.environ.get("GIRC_DOWNLOAD_TOKEN_MINUTES", cls.download_token_minutes)
            ),
            admin_email=os.environ.get("GIRC_ADMIN_EMAIL", cls.admin_email),
            max_upload_bytes=int(
                os.environ.get("GIRC_MAX_UPLOAD_BYTES", cls.max_upload_bytes)
            ),
            trust_forwarded_for=os.environ.get(
                "GIRC_TRUST_FORWARDED_FOR", ""
            ).lower()
            in {"1", "true", "yes", "on"},
            max_login_attempts=int(
                os.environ.get("GIRC_MAX_LOGIN_ATTEMPTS", cls.max_login_attempts)
            ),
            login_lockout_seconds=int(
                os.environ.get(
                    "GIRC_LOGIN_LOCKOUT_SECONDS", cls.login_lockout_seconds
                )
            ),
        )
