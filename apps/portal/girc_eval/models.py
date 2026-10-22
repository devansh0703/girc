"""The portal's data model.

The schema is deliberately small and explicit: organisations, their users,
the agreement they accept, the evaluations they request, the artifacts they
may download, the results they submit and the audit log that records every
state change.
"""

from __future__ import annotations

import enum
from datetime import datetime, timezone

from sqlalchemy import (
    Boolean,
    DateTime,
    Enum,
    ForeignKey,
    Integer,
    String,
    Text,
    UniqueConstraint,
)
from sqlalchemy.orm import DeclarativeBase, Mapped, mapped_column, relationship


def utcnow() -> datetime:
    """The current UTC time, as an aware datetime."""
    return datetime.now(timezone.utc)


class Base(DeclarativeBase):
    """The declarative base for every table."""


class RequestStatus(str, enum.Enum):
    """The lifecycle of an evaluation request."""

    submitted = "submitted"
    under_review = "under_review"
    approved = "approved"
    declined = "declined"
    complete = "complete"


class ResultStatus(str, enum.Enum):
    """The lifecycle of a result submission."""

    submitted = "submitted"
    under_review = "under_review"
    accepted = "accepted"
    rejected = "rejected"


class UserRole(str, enum.Enum):
    """What a user may do in the portal."""

    #: A user of a customer organisation.
    member = "member"
    #: A user who may sign the agreement and request evaluations.
    org_admin = "org_admin"
    #: A user of the portal operator.
    platform_admin = "platform_admin"


class Organization(Base):
    """A customer organisation."""

    __tablename__ = "organizations"

    id: Mapped[int] = mapped_column(primary_key=True)
    name: Mapped[str] = mapped_column(String(200), unique=True)
    domain: Mapped[str | None] = mapped_column(String(200), default=None)
    created_utc: Mapped[datetime] = mapped_column(DateTime, default=utcnow)

    users: Mapped[list["User"]] = relationship(
        back_populates="organization", cascade="all, delete-orphan"
    )
    agreements: Mapped[list["Agreement"]] = relationship(
        back_populates="organization", cascade="all, delete-orphan"
    )
    requests: Mapped[list["EvaluationRequest"]] = relationship(
        back_populates="organization", cascade="all, delete-orphan"
    )


class User(Base):
    """A person who can sign in."""

    __tablename__ = "users"

    id: Mapped[int] = mapped_column(primary_key=True)
    email: Mapped[str] = mapped_column(String(320), unique=True, index=True)
    full_name: Mapped[str] = mapped_column(String(200))
    #: The Argon2 password hash. The plaintext is never stored.
    password_hash: Mapped[str] = mapped_column(String(255))
    role: Mapped[UserRole] = mapped_column(Enum(UserRole), default=UserRole.member)
    organization_id: Mapped[int | None] = mapped_column(
        ForeignKey("organizations.id", ondelete="CASCADE"), default=None
    )
    is_active: Mapped[bool] = mapped_column(Boolean, default=True)
    created_utc: Mapped[datetime] = mapped_column(DateTime, default=utcnow)

    organization: Mapped[Organization | None] = relationship(back_populates="users")


class Agreement(Base):
    """An accepted confidential evaluation agreement.

    The agreement's text is versioned, and the accepted version's digest is
    recorded, so a later change to the document cannot silently alter what
    was agreed to.
    """

    __tablename__ = "agreements"

    id: Mapped[int] = mapped_column(primary_key=True)
    organization_id: Mapped[int] = mapped_column(
        ForeignKey("organizations.id", ondelete="CASCADE"), index=True
    )
    #: The version of the agreement text that was accepted.
    version: Mapped[str] = mapped_column(String(40))
    #: The SHA-256 digest of the agreement text.
    text_digest: Mapped[str] = mapped_column(String(64))
    #: The address the agreement was accepted from, for the audit record.
    accepted_from_ip: Mapped[str | None] = mapped_column(String(64), default=None)
    accepted_by_id: Mapped[int] = mapped_column(ForeignKey("users.id"))
    accepted_utc: Mapped[datetime] = mapped_column(DateTime, default=utcnow)

    organization: Mapped[Organization] = relationship(back_populates="agreements")
    accepted_by: Mapped[User] = relationship()

    __table_args__ = (UniqueConstraint("organization_id", "version"),)


class EvaluationRequest(Base):
    """A request to evaluate the mechanism against a customer's codec."""

    __tablename__ = "evaluation_requests"

    id: Mapped[int] = mapped_column(primary_key=True)
    organization_id: Mapped[int] = mapped_column(
        ForeignKey("organizations.id", ondelete="CASCADE"), index=True
    )
    #: A short human-readable reference, e.g. "EVAL-0007".
    reference: Mapped[str] = mapped_column(String(40), unique=True, index=True)
    title: Mapped[str] = mapped_column(String(200))
    #: The codec the customer wants to test against.
    target_codec: Mapped[str] = mapped_column(String(120))
    target_codec_version: Mapped[str | None] = mapped_column(String(120), default=None)
    #: The content the customer intends to test on.
    test_content_description: Mapped[str] = mapped_column(Text)
    #: The sampling plan the customer proposes.
    sampling_plan: Mapped[str] = mapped_column(Text)
    #: Why the customer wants the evaluation.
    motivation: Mapped[str] = mapped_column(Text)
    status: Mapped[RequestStatus] = mapped_column(
        Enum(RequestStatus), default=RequestStatus.submitted, index=True
    )
    #: The reviewer's note, when the request was decided.
    decision_note: Mapped[str | None] = mapped_column(Text, default=None)
    requested_by_id: Mapped[int] = mapped_column(ForeignKey("users.id"))
    requested_utc: Mapped[datetime] = mapped_column(DateTime, default=utcnow)
    decided_utc: Mapped[datetime | None] = mapped_column(DateTime, default=None)

    organization: Mapped[Organization] = relationship(back_populates="requests")
    requested_by: Mapped[User] = relationship()
    results: Mapped[list["ResultSubmission"]] = relationship(
        back_populates="request", cascade="all, delete-orphan"
    )


class Artifact(Base):
    """A gated artifact an approved organisation may download."""

    __tablename__ = "artifacts"

    id: Mapped[int] = mapped_column(primary_key=True)
    #: A stable machine name, e.g. "girc-core-source".
    slug: Mapped[str] = mapped_column(String(80), unique=True, index=True)
    title: Mapped[str] = mapped_column(String(200))
    description: Mapped[str] = mapped_column(Text)
    #: The path on disk, relative to the artifact root.
    stored_path: Mapped[str] = mapped_column(String(400))
    #: The SHA-256 digest of the file, checked on every download.
    sha256: Mapped[str] = mapped_column(String(64))
    size_bytes: Mapped[int] = mapped_column(Integer)
    #: The version of the artifact.
    version: Mapped[str] = mapped_column(String(40))
    #: The minimum agreement version an organisation must have accepted.
    requires_agreement_version: Mapped[str] = mapped_column(String(40))
    created_utc: Mapped[datetime] = mapped_column(DateTime, default=utcnow)


class ResultSubmission(Base):
    """A customer's evaluation results."""

    __tablename__ = "result_submissions"

    id: Mapped[int] = mapped_column(primary_key=True)
    request_id: Mapped[int] = mapped_column(
        ForeignKey("evaluation_requests.id", ondelete="CASCADE"), index=True
    )
    #: The label the customer gave the submission.
    label: Mapped[str] = mapped_column(String(200))
    #: The SHA-256 digest of the uploaded file.
    content_sha256: Mapped[str] = mapped_column(String(64))
    stored_path: Mapped[str] = mapped_column(String(400))
    size_bytes: Mapped[int] = mapped_column(Integer)
    #: The customer's own summary of the results.
    summary: Mapped[str] = mapped_column(Text)
    #: The BD-rate the customer measured, in percent, if they reported one.
    reported_bd_rate: Mapped[float | None] = mapped_column(default=None)
    status: Mapped[ResultStatus] = mapped_column(
        Enum(ResultStatus), default=ResultStatus.submitted, index=True
    )
    review_note: Mapped[str | None] = mapped_column(Text, default=None)
    submitted_by_id: Mapped[int] = mapped_column(ForeignKey("users.id"))
    submitted_utc: Mapped[datetime] = mapped_column(DateTime, default=utcnow)
    reviewed_utc: Mapped[datetime | None] = mapped_column(DateTime, default=None)

    request: Mapped[EvaluationRequest] = relationship(back_populates="results")
    submitted_by: Mapped[User] = relationship()


class AuditEntry(Base):
    """An append-only record of a state change.

    The log is the portal's evidence trail: every download, every status
    change and every agreement acceptance is recorded with the actor, the
    time and the address the action came from.
    """

    __tablename__ = "audit_entries"

    id: Mapped[int] = mapped_column(primary_key=True)
    organization_id: Mapped[int | None] = mapped_column(
        ForeignKey("organizations.id", ondelete="SET NULL"), default=None, index=True
    )
    actor_email: Mapped[str] = mapped_column(String(320), index=True)
    action: Mapped[str] = mapped_column(String(120), index=True)
    #: A human-readable description of what happened.
    detail: Mapped[str] = mapped_column(Text)
    #: The address the action came from.
    source_ip: Mapped[str | None] = mapped_column(String(64), default=None)
    created_utc: Mapped[datetime] = mapped_column(DateTime, default=utcnow, index=True)
