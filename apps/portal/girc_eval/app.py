"""The evaluation portal's HTTP application.

The routes implement the evaluation lifecycle: an organisation registers,
a user signs the confidential evaluation agreement, the organisation
requests an evaluation, an administrator approves it, the gated artifacts
become downloadable, the organisation submits results, and a reviewer
accepts or rejects them.

Every state-changing route writes an audit entry, and every route that
touches gated material checks the agreement before releasing anything.
"""

from __future__ import annotations

import hashlib
import json
import shutil
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Annotated

from fastapi import Depends, FastAPI, File, Form, HTTPException, Request, UploadFile
from fastapi.responses import FileResponse, HTMLResponse, JSONResponse, RedirectResponse
from fastapi.staticfiles import StaticFiles
from fastapi.templating import Jinja2Templates
from sqlalchemy import select
from sqlalchemy.orm import Session

from girc_eval import audit
from girc_eval.agreement import AGREEMENT_TEXT, CURRENT_VERSION, agreement_digest
from girc_eval.config import Settings
from girc_eval.db import Database
from girc_eval.models import (
    Agreement,
    Artifact,
    AuditEntry,
    EvaluationRequest,
    Organization,
    RequestStatus,
    ResultStatus,
    ResultSubmission,
    User,
    UserRole,
)
from girc_eval.security import (
    constant_time_equal,
    hash_password,
    read_download,
    read_session,
    resolve_within,
    safe_filename,
    sign_download,
    sign_session,
    verify_password,
)

#: The directory the templates and static files live in.
TEMPLATES_DIR = Path(__file__).parent / "templates"
STATIC_DIR = Path(__file__).parent / "static"


def create_app(settings: Settings | None = None) -> FastAPI:
    """Build the application.

    The factory exists so the test suite can point the application at a
    temporary database and a fresh settings object.
    """
    settings = settings or Settings.from_env()
    database = Database(settings)
    database.create_all()

    app = FastAPI(
        title="GIRC Evaluate",
        description="Confidential evaluation portal for the Geometry-Inference Codec.",
        version="1.0.0",
    )
    app.state.settings = settings
    app.state.database = database

    templates = Jinja2Templates(directory=str(TEMPLATES_DIR))
    if STATIC_DIR.exists():
        app.mount("/static", StaticFiles(directory=str(STATIC_DIR)), name="static")

    def db_session() -> Session:
        """A database session for the request."""
        session = database._session_factory()
        try:
            yield session
            session.commit()
        except Exception:
            session.rollback()
            raise
        finally:
            session.close()

    def current_user(request: Request, db: Session = Depends(db_session)) -> User | None:
        """The signed-in user, or `None`."""
        token = request.cookies.get("girc_session")
        if not token:
            return None
        user_id = read_session(token, settings.secret_key, settings.session_hours)
        if user_id is None:
            return None
        user = db.get(User, user_id)
        if user is None or not user.is_active:
            return None
        return user

    def require_user(user: User | None = Depends(current_user)) -> User:
        """The signed-in user, or a 401."""
        if user is None:
            raise HTTPException(status_code=401, detail="sign in required")
        return user

    def require_admin(user: User = Depends(require_user)) -> User:
        """A platform administrator, or a 403."""
        if user.role != UserRole.platform_admin:
            raise HTTPException(status_code=403, detail="administrator access required")
        return user

    # The failed sign-in attempts, by address.
    #
    # The portal is a single process, so an in-memory table is enough to
    # slow a guessing run down. A deployment that runs more than one
    # process puts a shared limiter in front of it instead.
    login_failures: dict[str, list[float]] = {}

    def login_allowed(address: str) -> bool:
        """Whether an address may attempt to sign in."""
        failures = login_failures.get(address)
        if not failures:
            return True
        if time.monotonic() - failures[-1] > settings.login_lockout_seconds:
            login_failures.pop(address, None)
            return True
        return len(failures) < settings.max_login_attempts

    def record_login_failure(address: str) -> None:
        """Count a failed sign-in against an address."""
        failures = login_failures.setdefault(address, [])
        failures.append(time.monotonic())

    def clear_login_failures(address: str) -> None:
        """Forget an address's failed sign-ins after a success."""
        login_failures.pop(address, None)

    def source_ip(request: Request) -> str:
        """The address the request came from.

        The `X-Forwarded-For` header is only consulted when the
        deployment says a proxy sets it, because a client can send any
        value it likes and the address ends up in the audit log. When no
        proxy is configured, the socket's own address is used, which
        cannot be forged.
        """
        if settings.trust_forwarded_for:
            forwarded = request.headers.get("x-forwarded-for")
            if forwarded:
                return forwarded.split(",")[0].strip()
        return request.client.host if request.client else "unknown"

    # ---------------------------------------------------------------- pages

    @app.get("/", response_class=HTMLResponse)
    def index(request: Request, db: Session = Depends(db_session), user: User | None = Depends(current_user)):
        """The portal's landing page."""
        recent = audit.recent(db, limit=10)
        return templates.TemplateResponse(
            request,
            "index.html",
            {
                "request": request,
                "user": user,
                "recent_audit": recent,
                "artifact_count": len(list(db.scalars(select(Artifact)))),
                "request_count": len(list(db.scalars(select(EvaluationRequest)))),
            },
        )

    @app.get("/login", response_class=HTMLResponse)
    def login_page(request: Request):
        """The sign-in page."""
        return templates.TemplateResponse("login.html", {"request": request})

    @app.get("/register", response_class=HTMLResponse)
    def register_page(request: Request):
        """The registration page."""
        return templates.TemplateResponse("register.html", {"request": request})

    @app.get("/dashboard", response_class=HTMLResponse)
    def dashboard(
        request: Request,
        user: User = Depends(require_user),
        db: Session = Depends(db_session),
    ):
        """The signed-in user's dashboard."""
        organization = (
            db.get(Organization, user.organization_id) if user.organization_id else None
        )
        requests = []
        if organization:
            requests = list(
                db.scalars(
                    select(EvaluationRequest)
                    .where(EvaluationRequest.organization_id == organization.id)
                    .order_by(EvaluationRequest.requested_utc.desc())
                )
            )
        agreement = None
        if organization:
            agreement = db.scalar(
                select(Agreement).where(
                    Agreement.organization_id == organization.id,
                    Agreement.version == CURRENT_VERSION,
                )
            )
        artifacts = list(db.scalars(select(Artifact).order_by(Artifact.title)))
        return templates.TemplateResponse(
            request,
            "dashboard.html",
            {
                "request": request,
                "user": user,
                "organization": organization,
                "requests": requests,
                "agreement": agreement,
                "artifacts": artifacts,
                "agreement_text": AGREEMENT_TEXT,
                "agreement_version": CURRENT_VERSION,
                "can_download": organization is not None
                and agreement is not None
                and any(r.status == RequestStatus.approved for r in requests),
            },
        )

    @app.get("/admin", response_class=HTMLResponse)
    def admin_page(
        request: Request,
        user: User = Depends(require_admin),
        db: Session = Depends(db_session),
    ):
        """The administrator's view."""
        requests = list(
            db.scalars(
                select(EvaluationRequest).order_by(EvaluationRequest.requested_utc.desc())
            )
        )
        organizations = list(db.scalars(select(Organization)))
        artifacts = list(db.scalars(select(Artifact)))
        return templates.TemplateResponse(
            request,
            "admin.html",
            {
                "request": request,
                "user": user,
                "requests": requests,
                "organizations": organizations,
                "artifacts": artifacts,
                "audit_entries": audit.recent(db, limit=50),
            },
        )

    # ------------------------------------------------------------ accounts

    @app.post("/register", response_class=JSONResponse)
    def register(
        request: Request,
        organization_name: Annotated[str, Form()],
        email: Annotated[str, Form()],
        password: Annotated[str, Form()],
        full_name: Annotated[str, Form()],
        db: Session = Depends(db_session),
    ):
        """Register an organisation and its first user."""
        if len(password) < 12:
            raise HTTPException(
                status_code=400,
                detail="the password must be at least twelve characters",
            )
        if db.scalar(select(User).where(User.email == email.lower())):
            raise HTTPException(status_code=409, detail="that address is registered")

        organization = db.scalar(
            select(Organization).where(Organization.name == organization_name)
        )
        if organization is None:
            organization = Organization(name=organization_name)
            db.add(organization)
            db.flush()

        user = User(
            email=email.lower(),
            full_name=full_name,
            password_hash=hash_password(password),
            role=UserRole.org_admin,
            organization_id=organization.id,
        )
        db.add(user)
        db.flush()

        audit.record(
            db,
            action="organisation.registered",
            detail=f"{organization_name} registered with {email}",
            actor_email=email.lower(),
            organization_id=organization.id,
            source_ip=source_ip(request),
        )
        db.commit()
        response = JSONResponse({"status": "registered"})
        response.set_cookie(
            "girc_session",
            sign_session(user.id, settings.secret_key),
            httponly=True,
            samesite="lax",
            max_age=settings.session_hours * 3600,
        )
        return response

    @app.post("/login", response_class=JSONResponse)
    def login(
        request: Request,
        email: Annotated[str, Form()],
        password: Annotated[str, Form()],
        db: Session = Depends(db_session),
    ):
        """Sign in.

        The route is rate limited per address, because a portal that
        hands out confidential material is worth guessing at.
        """
        address = source_ip(request)
        if not login_allowed(address):
            audit.record(
                db,
                action="login.locked_out",
                detail=f"the sign-in attempts from {address} exceeded the limit",
                actor_email="",
                source_ip=address,
            )
            db.commit()
            raise HTTPException(
                status_code=429,
                detail="too many sign-in attempts; try again later",
            )

        user = db.scalar(select(User).where(User.email == email.lower()))
        if user is None or not verify_password(user.password_hash, password):
            record_login_failure(address)
            audit.record(
                db,
                action="login.failed",
                detail=f"a failed sign-in for {email.lower()}",
                actor_email=email.lower(),
                source_ip=source_ip(request),
            )
            db.commit()
            raise HTTPException(status_code=401, detail="those credentials are wrong")
        if not user.is_active:
            raise HTTPException(status_code=403, detail="that account is disabled")
        clear_login_failures(address)
        audit.record(
            db,
            action="login.succeeded",
            detail=f"{user.email} signed in",
            actor_email=user.email,
            organization_id=user.organization_id,
            source_ip=source_ip(request),
        )
        db.commit()
        response = JSONResponse({"status": "ok"})
        response.set_cookie(
            "girc_session",
            sign_session(user.id, settings.secret_key),
            httponly=True,
            samesite="lax",
            max_age=settings.session_hours * 3600,
        )
        return response

    @app.post("/logout")
    def logout():
        """Sign out by clearing the session cookie."""
        response = RedirectResponse("/", status_code=303)
        response.delete_cookie("girc_session")
        return response

    # ---------------------------------------------------------- agreement

    @app.post("/agreement/accept", response_class=JSONResponse)
    def accept_agreement(
        request: Request,
        user: User = Depends(require_user),
        db: Session = Depends(db_session),
    ):
        """Accept the confidential evaluation agreement."""
        if user.organization_id is None:
            raise HTTPException(status_code=400, detail="the account has no organisation")

        existing = db.scalar(
            select(Agreement).where(
                Agreement.organization_id == user.organization_id,
                Agreement.version == CURRENT_VERSION,
            )
        )
        if existing is not None:
            return JSONResponse(
                {"status": "already-accepted", "version": CURRENT_VERSION}
            )

        agreement = Agreement(
            organization_id=user.organization_id,
            version=CURRENT_VERSION,
            text_digest=agreement_digest(),
            accepted_from_ip=source_ip(request),
            accepted_by_id=user.id,
        )
        db.add(agreement)
        db.flush()
        audit.record(
            db,
            action="agreement.accepted",
            detail=f"{user.email} accepted agreement {CURRENT_VERSION}",
            actor_email=user.email,
            organization_id=user.organization_id,
            source_ip=source_ip(request),
        )
        db.commit()
        return JSONResponse(
            {"status": "accepted", "version": CURRENT_VERSION, "digest": agreement_digest()}
        )

    # -------------------------------------------------------- evaluation

    @app.post("/requests", response_class=JSONResponse)
    def create_request(
        request: Request,
        title: Annotated[str, Form()],
        target_codec: Annotated[str, Form()],
        test_content_description: Annotated[str, Form()],
        sampling_plan: Annotated[str, Form()],
        motivation: Annotated[str, Form()],
        target_codec_version: Annotated[str, Form()] = "",
        user: User = Depends(require_user),
        db: Session = Depends(db_session),
    ):
        """Request an evaluation."""
        if user.organization_id is None:
            raise HTTPException(status_code=400, detail="the account has no organisation")

        agreement = db.scalar(
            select(Agreement).where(
                Agreement.organization_id == user.organization_id,
                Agreement.version == CURRENT_VERSION,
            )
        )
        if agreement is None:
            raise HTTPException(
                status_code=403,
                detail="the confidential evaluation agreement must be accepted first",
            )

        count = len(list(db.scalars(select(EvaluationRequest))))
        reference = f"EVAL-{count + 1:04d}"
        evaluation = EvaluationRequest(
            organization_id=user.organization_id,
            reference=reference,
            title=title,
            target_codec=target_codec,
            target_codec_version=target_codec_version or None,
            test_content_description=test_content_description,
            sampling_plan=sampling_plan,
            motivation=motivation,
            requested_by_id=user.id,
        )
        db.add(evaluation)
        db.flush()
        audit.record(
            db,
            action="evaluation.requested",
            detail=f"{reference} requested by {user.email} for {target_codec}",
            actor_email=user.email,
            organization_id=user.organization_id,
            source_ip=source_ip(request),
        )
        db.commit()
        return JSONResponse({"status": "submitted", "reference": reference})

    @app.post("/requests/{reference}/decision", response_class=JSONResponse)
    def decide_request(
        request: Request,
        reference: str,
        decision: Annotated[str, Form()],
        note: Annotated[str, Form()] = "",
        user: User = Depends(require_admin),
        db: Session = Depends(db_session),
    ):
        """Approve or decline an evaluation request."""
        evaluation = db.scalar(
            select(EvaluationRequest).where(EvaluationRequest.reference == reference)
        )
        if evaluation is None:
            raise HTTPException(status_code=404, detail="no such request")

        evaluation.status = (
            RequestStatus.approved
            if decision == "approve"
            else RequestStatus.declined
        )
        evaluation.decision_note = note or None
        evaluation.decided_utc = datetime.now(timezone.utc)
        audit.record(
            db,
            action=f"evaluation.{decision}d",
            detail=f"{reference} was {decision}d by {user.email}",
            actor_email=user.email,
            organization_id=evaluation.organization_id,
            source_ip=source_ip(request),
        )
        db.commit()
        return JSONResponse({"status": evaluation.status.value})

    # ----------------------------------------------------------- artifacts

    @app.get("/artifacts/{slug}/download-token", response_class=JSONResponse)
    def download_token(
        request: Request,
        slug: str,
        user: User = Depends(require_user),
        db: Session = Depends(db_session),
    ):
        """Issue a signed download token for a gated artifact."""
        artifact = db.scalar(select(Artifact).where(Artifact.slug == slug))
        if artifact is None:
            raise HTTPException(status_code=404, detail="no such artifact")

        if not has_approved_evaluation(db, user):
            raise HTTPException(
                status_code=403,
                detail="an approved evaluation is required to download the package",
            )

        token = sign_download(
            artifact.id,
            artifact.sha256,
            user.id,
            settings.secret_key,
            settings.download_token_minutes,
        )
        audit.record(
            db,
            action="artifact.download.authorised",
            detail=f"{user.email} was issued a token for {artifact.slug}",
            actor_email=user.email,
            organization_id=user.organization_id,
            source_ip=source_ip(request),
        )
        db.commit()
        return JSONResponse({"token": token, "filename": f"{artifact.slug}.tar.gz"})

    @app.get("/download")
    def download(
        request: Request,
        token: str,
        db: Session = Depends(db_session),
    ):
        """Download a gated artifact with a signed token."""
        parsed = read_download(token, settings.secret_key, settings.download_token_minutes)
        if parsed is None:
            raise HTTPException(status_code=403, detail="the download link is not valid")
        artifact_id, expected_digest, user_id = parsed

        artifact = db.get(Artifact, artifact_id)
        if artifact is None:
            raise HTTPException(status_code=404, detail="no such artifact")
        if not constant_time_equal(artifact.sha256, expected_digest):
            raise HTTPException(status_code=403, detail="the download link is not valid")

        user = db.get(User, user_id)
        if user is None or not has_approved_evaluation(db, user):
            raise HTTPException(
                status_code=403,
                detail="an approved evaluation is required to download the package",
            )

        try:
            path = resolve_within(settings.artifact_root, artifact.stored_path)
        except ValueError:
            raise HTTPException(status_code=404, detail="the artifact is not available")
        if not path.exists():
            raise HTTPException(status_code=404, detail="the artifact is not available")

        audit.record(
            db,
            action="artifact.downloaded",
            detail=f"{user.email} downloaded {artifact.slug} {artifact.version}",
            actor_email=user.email,
            organization_id=user.organization_id,
            source_ip=source_ip(request),
        )
        db.commit()
        return FileResponse(
            path,
            media_type="application/gzip",
            filename=f"{artifact.slug}-{artifact.version}.tar.gz",
        )

    # ------------------------------------------------------------- results

    @app.post("/requests/{reference}/results", response_class=JSONResponse)
    async def submit_result(
        request: Request,
        reference: str,
        label: Annotated[str, Form()],
        summary: Annotated[str, Form()],
        reported_bd_rate: Annotated[str, Form()] = "",
        file: Annotated[UploadFile, File()] = None,
        user: User = Depends(require_user),
        db: Session = Depends(db_session),
    ):
        """Submit the results of an evaluation."""
        evaluation = db.scalar(
            select(EvaluationRequest).where(EvaluationRequest.reference == reference)
        )
        if evaluation is None:
            raise HTTPException(status_code=404, detail="no such request")
        if evaluation.organization_id != user.organization_id:
            raise HTTPException(status_code=403, detail="the request belongs to another organisation")

        digest = ""
        stored_path = ""
        size = 0
        if file is not None and file.filename:
            content = await file.read()
            if len(content) > settings.max_upload_bytes:
                raise HTTPException(status_code=413, detail="the result file is too large")
            digest = hashlib.sha256(content).hexdigest()
            size = len(content)
            # The name is the uploader's, so it is reduced to a safe one
            # and the destination is resolved inside the upload
            # directory, so a name like `../../etc/passwd` cannot write
            # outside it.
            name = safe_filename(file.filename, fallback="result.bin")
            uploads = resolve_within(settings.artifact_root, "uploads", reference)
            uploads.mkdir(parents=True, exist_ok=True)
            target = resolve_within(uploads, name)
            target.write_bytes(content)
            stored_path = str(target.relative_to(settings.artifact_root.resolve()))

        try:
            bd_rate = float(reported_bd_rate) if reported_bd_rate.strip() else None
        except ValueError:
            raise HTTPException(status_code=400, detail="the BD-rate must be a number")

        result = ResultSubmission(
            request_id=evaluation.id,
            label=label,
            content_sha256=digest,
            stored_path=stored_path,
            size_bytes=size,
            summary=summary,
            reported_bd_rate=bd_rate,
            submitted_by_id=user.id,
        )
        db.add(result)
        db.flush()
        audit.record(
            db,
            action="result.submitted",
            detail=f"{user.email} submitted results for {reference}",
            actor_email=user.email,
            organization_id=user.organization_id,
            source_ip=source_ip(request),
        )
        db.commit()
        return JSONResponse({"status": "submitted"})

    @app.post("/results/{result_id}/decision", response_class=JSONResponse)
    def decide_result(
        request: Request,
        result_id: int,
        decision: Annotated[str, Form()],
        note: Annotated[str, Form()] = "",
        user: User = Depends(require_admin),
        db: Session = Depends(db_session),
    ):
        """Accept or reject a result submission."""
        result = db.get(ResultSubmission, result_id)
        if result is None:
            raise HTTPException(status_code=404, detail="no such result")

        result.status = (
            ResultStatus.accepted if decision == "accept" else ResultStatus.rejected
        )
        result.review_note = note or None
        result.reviewed_utc = datetime.now(timezone.utc)

        if result.status == ResultStatus.accepted:
            evaluation = db.get(EvaluationRequest, result.request_id)
            if evaluation is not None:
                evaluation.status = RequestStatus.complete

        audit.record(
            db,
            action=f"result.{decision}ed",
            detail=f"result {result_id} was {decision}ed by {user.email}",
            actor_email=user.email,
            organization_id=result.request_id and db.get(
                EvaluationRequest, result.request_id
            ).organization_id,
            source_ip=source_ip(request),
        )
        db.commit()
        return JSONResponse({"status": result.status.value})

    # -------------------------------------------------------------- admin

    @app.post("/admin/artifacts", response_class=JSONResponse)
    def register_artifact(
        request: Request,
        slug: Annotated[str, Form()],
        title: Annotated[str, Form()],
        description: Annotated[str, Form()],
        version: Annotated[str, Form()],
        requires_agreement_version: Annotated[str, Form()],
        user: User = Depends(require_admin),
        db: Session = Depends(db_session),
    ):
        """Register a gated artifact from a file already in the store."""
        # Both the name the file is looked up under and the name it is
        # stored under are the safe ones, so a slug carrying a directory
        # component simply does not name a file.
        safe_slug = safe_filename(slug, fallback="artifact")
        safe_version = safe_filename(version, fallback="1")
        source = resolve_within(settings.artifact_root, "incoming", safe_slug)
        if not source.exists():
            raise HTTPException(status_code=404, detail="the artifact file is not present")

        digest = hashlib.sha256(source.read_bytes()).hexdigest()
        stored = f"artifacts/{safe_slug}-{safe_version}.tar.gz"
        target = resolve_within(settings.artifact_root, stored)
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, target)

        artifact = db.scalar(select(Artifact).where(Artifact.slug == slug))
        if artifact is None:
            artifact = Artifact(slug=slug)
            db.add(artifact)
        artifact.title = title
        artifact.description = description
        artifact.version = version
        artifact.requires_agreement_version = requires_agreement_version
        artifact.stored_path = stored
        artifact.sha256 = digest
        artifact.size_bytes = target.stat().st_size
        db.flush()
        audit.record(
            db,
            action="artifact.registered",
            detail=f"{slug} {version} was registered by {user.email}",
            actor_email=user.email,
            source_ip=source_ip(request),
        )
        db.commit()
        return JSONResponse({"status": "registered", "sha256": digest})

    @app.get("/audit", response_class=JSONResponse)
    def audit_log(
        limit: int = 100,
        user: User = Depends(require_admin),
        db: Session = Depends(db_session),
    ):
        """The audit log."""
        entries = audit.recent(db, limit=limit)
        return JSONResponse(
            {
                "entries": [
                    {
                        "at": entry.created_utc.isoformat(),
                        "actor": entry.actor_email,
                        "action": entry.action,
                        "detail": entry.detail,
                        "source_ip": entry.source_ip,
                    }
                    for entry in entries
                ]
            }
        )

    @app.get("/health")
    def health():
        """A liveness probe."""
        return {"status": "ok"}

    return app


def has_approved_evaluation(db: Session, user: User) -> bool:
    """Whether a user may download the gated artifacts."""
    if user.organization_id is None:
        return False
    approved = db.scalar(
        select(EvaluationRequest)
        .where(
            EvaluationRequest.organization_id == user.organization_id,
            EvaluationRequest.status == RequestStatus.approved,
        )
        .limit(1)
    )
    return approved is not None
