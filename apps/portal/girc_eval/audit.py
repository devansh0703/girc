"""The audit log.

Every state change in the portal is recorded: who did it, when, from where
and what happened. The log is append-only, and the application's own code
never updates or deletes a row in it.
"""

from __future__ import annotations

from sqlalchemy import func, select
from sqlalchemy.orm import Session

from girc_eval.models import AuditEntry


def record(
    session: Session,
    *,
    action: str,
    detail: str,
    actor_email: str,
    organization_id: int | None = None,
    source_ip: str | None = None,
) -> AuditEntry:
    """Append an entry to the audit log."""
    entry = AuditEntry(
        action=action,
        detail=detail,
        actor_email=actor_email,
        organization_id=organization_id,
        source_ip=source_ip,
    )
    session.add(entry)
    return entry


def entries_for_organization(session: Session, organization_id: int) -> list[AuditEntry]:
    """Every audit entry for one organisation, newest first."""
    statement = (
        select(AuditEntry)
        .where(AuditEntry.organization_id == organization_id)
        .order_by(AuditEntry.created_utc.desc(), AuditEntry.id.desc())
    )
    return list(session.scalars(statement))


def recent(session: Session, limit: int = 100) -> list[AuditEntry]:
    """The most recent audit entries across all organisations."""
    statement = select(AuditEntry).order_by(
        AuditEntry.created_utc.desc(), AuditEntry.id.desc()
    ).limit(limit)
    return list(session.scalars(statement))


def count(session: Session) -> int:
    """The number of audit entries."""
    return int(session.scalar(select(func.count()).select_from(AuditEntry)) or 0)
