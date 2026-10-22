"""Database access for the evaluation portal.

The portal uses SQLAlchemy 2.0 with a session-per-request pattern. The
default database is a local SQLite file; the schema is created on first use
so a fresh checkout runs with no migration step.
"""

from __future__ import annotations

from collections.abc import Iterator
from contextlib import contextmanager
from pathlib import Path

from sqlalchemy import create_engine, event
from sqlalchemy.engine import Engine
from sqlalchemy.orm import Session, sessionmaker

from girc_eval.config import Settings
from girc_eval.models import Base


class Database:
    """The portal's database handle."""

    def __init__(self, settings: Settings | None = None) -> None:
        self.settings = settings or Settings()
        self.engine: Engine = self._create_engine(self.settings.database_url)
        self._session_factory = sessionmaker(
            bind=self.engine, autoflush=False, expire_on_commit=False
        )

    @staticmethod
    def _create_engine(url: str) -> Engine:
        """Create the engine, enabling foreign keys for SQLite.

        SQLite does not enforce foreign keys unless asked to, and the
        portal's cascade behaviour depends on them.
        """
        connect_args = {}
        if url.startswith("sqlite"):
            connect_args["check_same_thread"] = False

        engine = create_engine(url, connect_args=connect_args, future=True)

        if url.startswith("sqlite"):

            @event.listens_for(engine, "connect")
            def _enable_foreign_keys(dbapi_connection, _record):  # pragma: no cover
                cursor = dbapi_connection.cursor()
                cursor.execute("PRAGMA foreign_keys=ON")
                cursor.close()

        return engine

    def create_all(self) -> None:
        """Create the schema if it does not exist."""
        # A SQLite file's parent directory has to exist first.
        url = self.settings.database_url
        if url.startswith("sqlite:///") and not url.startswith("sqlite:///:memory:"):
            path = Path(url.removeprefix("sqlite:///"))
            path.parent.mkdir(parents=True, exist_ok=True)
        Base.metadata.create_all(self.engine)

    def drop_all(self) -> None:
        """Drop every table. Used by the test suite's teardown."""
        Base.metadata.drop_all(self.engine)

    @contextmanager
    def session(self) -> Iterator[Session]:
        """A session that commits on success and rolls back on failure."""
        session = self._session_factory()
        try:
            yield session
            session.commit()
        except Exception:
            session.rollback()
            raise
        finally:
            session.close()

    def dispose(self) -> None:
        """Release the engine's resources."""
        self.engine.dispose()
