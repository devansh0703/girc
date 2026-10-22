"""Security helpers: password hashing, session cookies and download tokens.

The portal handles confidential material, so the primitives are chosen
carefully:

* Passwords are hashed with Argon2id through `argon2-cffi`. The plaintext is
  never stored or logged.
* Sessions are signed with `itsdangerous`, so a session cookie cannot be
  forged without the secret key, and carries its own expiry.
* Download links are signed with an expiry and the artifact's digest, so a
  link cannot be replayed after it expires and cannot be edited to point at a
  different artifact.
"""

from __future__ import annotations

import hashlib
import hmac
import secrets
from datetime import datetime, timezone

from argon2 import PasswordHasher
from argon2.exceptions import InvalidHashError, VerifyMismatchError
from itsdangerous import BadSignature, SignatureExpired, URLSafeTimedSerializer

#: The Argon2id parameters. The defaults are the library's, which follow
#: the OWASP recommendation for Argon2id.
_hasher = PasswordHasher()


def hash_password(password: str) -> str:
    """Hash a password with Argon2id."""
    return _hasher.hash(password)


def verify_password(password_hash: str, password: str) -> bool:
    """Check a password against its hash.

    A malformed hash is a failed verification, not an error: a corrupt row
    must not lock a user out of a check, and must not reveal that the row
    exists.
    """
    try:
        _hasher.verify(password_hash, password)
        return True
    except (VerifyMismatchError, InvalidHashError):
        return False


def needs_rehash(password_hash: str) -> bool:
    """Whether a stored hash should be upgraded to the current parameters."""
    try:
        return _hasher.check_needs_rehash(password_hash)
    except InvalidHashError:
        return False


def generate_token(nbytes: int = 32) -> str:
    """A cryptographically secure random token."""
    return secrets.token_urlsafe(nbytes)


def sha256_bytes(data: bytes) -> str:
    """The SHA-256 digest of some bytes, as lowercase hex."""
    return hashlib.sha256(data).hexdigest()


def sha256_file(path) -> str:
    """The SHA-256 digest of a file, as lowercase hex."""
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def constant_time_equal(left: str, right: str) -> bool:
    """Compare two strings without leaking their contents through timing."""
    return hmac.compare_digest(left.encode("utf-8"), right.encode("utf-8"))


def _serializer(secret_key: str, salt: str) -> URLSafeTimedSerializer:
    return URLSafeTimedSerializer(secret_key, salt=salt)


def sign_session(user_id: int, secret_key: str) -> str:
    """Sign a session cookie for a user."""
    serializer = _serializer(secret_key, "girc-eval-session")
    return serializer.dumps({"user_id": user_id})


def read_session(token: str, secret_key: str, max_age_hours: int) -> int | None:
    """The user id a session token names, or `None` if it is not valid."""
    serializer = _serializer(secret_key, "girc-eval-session")
    try:
        # itsdangerous takes the maximum age in seconds.
        payload = serializer.loads(token, max_age=max_age_hours * 3600)
    except (BadSignature, SignatureExpired):
        return None
    user_id = payload.get("user_id")
    return int(user_id) if isinstance(user_id, int) else None


def sign_download(
    artifact_id: int,
    artifact_sha256: str,
    user_id: int,
    secret_key: str,
    minutes: int,
) -> str:
    """Sign a download token for an artifact.

    The token binds the artifact, its digest and the user, so it cannot be
    edited to fetch a different artifact or reused by another user.
    """
    serializer = _serializer(secret_key, "girc-eval-download")
    payload = {
        "artifact_id": artifact_id,
        "sha256": artifact_sha256,
        "user_id": user_id,
        "issued": datetime.now(timezone.utc).isoformat(),
    }
    return serializer.dumps(payload)


def read_download(
    token: str, secret_key: str, minutes: int
) -> tuple[int, str, int] | None:
    """The `(artifact_id, sha256, user_id)` a download token names.

    `None` is returned when the token is invalid or has expired.
    """
    serializer = _serializer(secret_key, "girc-eval-download")
    try:
        payload = serializer.loads(token, max_age=minutes * 60)
    except (BadSignature, SignatureExpired):
        return None
    try:
        artifact_id = int(payload["artifact_id"])
        sha256 = str(payload["sha256"])
        user_id = int(payload["user_id"])
    except (KeyError, TypeError, ValueError):
        return None
    return artifact_id, sha256, user_id
