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
        )
