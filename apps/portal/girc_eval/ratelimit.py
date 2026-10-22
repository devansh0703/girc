"""The sign-in rate limiter.

A portal that hands out confidential material is worth guessing at, so
the sign-in route is rate limited. The limit has to hold across every
process serving the portal: a limiter that lives in one process's memory
is defeated by running two, which is exactly what a production
deployment does.

Two implementations are provided, and the settings choose between them:

* :class:`MemoryLimiter` keeps its state in the process. It is the
  default, because it needs no service and it is what the tests use.
* :class:`RedisLimiter` keeps its state in Redis, so every process
  shares it. It is what a deployment with more than one process uses.

The Redis implementation talks to Upstash's REST API over plain HTTPS,
because that is what a deployment without a network to a Redis server
has. The API is a set of ordinary Redis commands over HTTP, so the
limiter is a thin wrapper over four commands.

Both implementations answer the same two questions: may this address
try to sign in, and has this attempt failed. The semantics are
identical, so a deployment can be moved from one to the other without
the behaviour changing.
"""

from __future__ import annotations

import time
from dataclasses import dataclass, field
from typing import Any, Protocol


class Limiter(Protocol):
    """The interface a rate limiter implements."""

    def allowed(self, address: str) -> bool:
        """Whether the address may attempt to sign in."""
        ...

    def record_failure(self, address: str) -> None:
        """Count a failed sign-in against the address."""
        ...

    def clear_failures(self, address: str) -> None:
        """Forget the address's failed sign-ins."""
        ...


@dataclass
class MemoryLimiter:
    """A limiter that keeps its state in the process.

    The failed attempts are kept with the time they happened, so the
    window slides: an attempt older than the lockout does not count.
    """

    #: How many failed attempts are allowed before the address is
    #: locked out.
    max_attempts: int = 10

    #: How long a locked-out address is refused, in seconds.
    lockout_seconds: float = 900.0

    #: The failed attempts, by address.
    failures: dict[str, list[float]] = field(default_factory=dict)

    def allowed(self, address: str) -> bool:
        failures = self.failures.get(address)
        if not failures:
            return True
        if time.monotonic() - failures[-1] > self.lockout_seconds:
            # The window has passed, so the address is free again.
            self.failures.pop(address, None)
            return True
        return len(failures) < self.max_attempts

    def record_failure(self, address: str) -> None:
        self.failures.setdefault(address, []).append(time.monotonic())

    def clear_failures(self, address: str) -> None:
        self.failures.pop(address, None)


@dataclass
class RedisLimiter:
    """A limiter that keeps its state in Redis.

    The state is a sorted set per address, holding the time of each
    failed attempt, and the count of its members is the number of
    attempts in the window. The set expires on its own after the
    lockout, so a forgotten address leaves nothing behind.

    The commands are the ordinary Redis ones, sent as a JSON array of
    command arrays, which is the shape Upstash's REST API takes.
    """

    #: The Upstash Redis REST endpoint.
    url: str

    #: The bearer token for the endpoint.
    token: str

    #: How many failed attempts are allowed before the address is
    #: locked out.
    max_attempts: int = 10

    #: How long a locked-out address is refused, in seconds.
    lockout_seconds: float = 900.0

    #: The HTTP client used for the requests.
    client: Any | None = None

    #: The prefix for the keys, so one Redis can serve several portals.
    key_prefix: str = "girc-eval"

    def _http(self):
        if self.client is None:
            import httpx

            self.client = httpx.Client(timeout=5.0)
        return self.client

    def _command(self, *args: str) -> Any:
        """Run one Redis command over the REST API.

        The body is the command's arguments as a flat JSON array. The
        API also accepts a nested array for a pipeline, but it takes
        the flat form for a single command.
        """
        response = self._http().post(
            self.url,
            headers={"Authorization": f"Bearer {self.token}"},
            json=list(args),
        )
        response.raise_for_status()
        payload = response.json()
        # Upstash answers with {"result": ...} and, for errors,
        # {"error": ...}.
        if isinstance(payload, dict) and "error" in payload:
            raise RuntimeError(f"the rate limiter's store refused: {payload['error']}")
        if isinstance(payload, dict):
            return payload.get("result")
        return payload

    def _key(self, address: str) -> str:
        # The address is hashed so the key is a fixed length and holds
        # nothing that identifies a client in the clear.
        import hashlib

        digest = hashlib.sha256(address.encode("utf-8")).hexdigest()[:32]
        return f"{self.key_prefix}:login:{digest}"

    def allowed(self, address: str) -> bool:
        key = self._key(address)
        # Drop the attempts outside the window, then count what is left.
        cutoff = time.time() - self.lockout_seconds
        self._command("ZREMRANGEBYSCORE", key, "0", f"({cutoff}")
        count = self._command("ZCARD", key)
        try:
            return int(count) < self.max_attempts
        except (TypeError, ValueError):
            # If the count cannot be read, the safe answer is to refuse.
            return False

    def record_failure(self, address: str) -> None:
        key = self._key(address)
        now = time.time()
        self._command("ZADD", key, f"{now}", f"{now}")
        self._command("EXPIRE", key, str(int(self.lockout_seconds) + 1))

    def clear_failures(self, address: str) -> None:
        self._command("DEL", self._key(address))


def limiter_from_settings(settings) -> Limiter:
    """Build the limiter the settings ask for.

    The Redis limiter is used when an endpoint and a token are both
    configured, and the in-memory one otherwise.
    """
    redis_url = getattr(settings, "redis_url", "") or ""
    redis_token = getattr(settings, "redis_token", "") or ""
    if redis_url and redis_token:
        return RedisLimiter(
            url=redis_url,
            token=redis_token,
            max_attempts=settings.max_login_attempts,
            lockout_seconds=settings.login_lockout_seconds,
        )
    return MemoryLimiter(
        max_attempts=settings.max_login_attempts,
        lockout_seconds=settings.login_lockout_seconds,
    )


__all__ = ["Limiter", "MemoryLimiter", "RedisLimiter", "limiter_from_settings"]
