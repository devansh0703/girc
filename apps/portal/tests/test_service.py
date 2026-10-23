"""Tests for the hosted analysis service.

The service is a transport in front of the codec, so the tests are about the
transport: that it publishes the limits it enforces, that it refuses a body
that is not a sequence rather than reporting a measurement of nothing, that a
request leaves nothing behind, and that the report it returns is the shape the
benchmark writes.

The codec itself is covered by the Rust test suite. What is tested here is
that the service does not add a second, worse implementation of the
measurement.

Run: PYTHONPATH=apps/portal pytest apps/portal/tests -q
"""

from __future__ import annotations

import json
from pathlib import Path

import pytest
from fastapi.testclient import TestClient

from girc_eval import service

ROOT = Path(__file__).resolve().parents[3]
SEQUENCE = ROOT / "data" / "test" / "akiyo-192x192.y4m"


@pytest.fixture(scope="module")
def client() -> TestClient:
    return TestClient(service.app)


@pytest.fixture(scope="module")
def sequence() -> bytes:
    if not SEQUENCE.exists():
        pytest.skip(f"no test sequence at {SEQUENCE}")
    return SEQUENCE.read_bytes()


def test_the_limits_are_the_published_limits(client: TestClient) -> None:
    """Every limit the service enforces is reported on `/health`."""
    health = client.get("/health").json()
    for name, value in service.LIMITS.items():
        assert health[name] == value, f"the published limit {name} does not match"


def test_the_uptime_is_a_number(client: TestClient) -> None:
    """The uptime is reported, so a spinning instance is distinguishable."""
    assert isinstance(client.get("/health").json()["uptime_s"], int)


def test_the_index_is_a_form(client: TestClient) -> None:
    """A person can try the service without reading an API."""
    page = client.get("/")
    assert page.status_code == 200
    assert "multipart/form-data" in page.text
    assert "analyze" in page.text


def test_a_body_that_is_not_a_sequence_is_refused(client: TestClient) -> None:
    """A measurement of nothing is a 422, not a report with zeros in it."""
    response = client.post(
        "/analyze", files={"sequence": ("s.y4m", b"this is not a y4m file")}
    )
    assert response.status_code == 422
    assert "Y4M" in response.json()["detail"]


def test_an_empty_body_is_a_bad_request(client: TestClient) -> None:
    """An empty body is the caller's error, not the service's."""
    assert client.post("/analyze", content=b"").status_code == 400


def test_an_oversized_body_is_refused_by_the_published_limit(client: TestClient) -> None:
    """The body cap is the one published on `/health`, and it is enforced."""
    response = client.post(
        "/analyze",
        files={
            "sequence": (
                "s.y4m",
                b"YUV4MPEG2 W16 H16\nFRAME\n" + b"x" * (service.LIMITS["body_limit_bytes"] + 1),
            )
        },
    )
    assert response.status_code == 422
    assert "limit" in response.json()["detail"]


def test_the_report_is_the_shape_the_benchmark_writes(
    client: TestClient, sequence: bytes
) -> None:
    """A hosted run can be compared against the committed report directly."""
    response = client.post(
        "/analyze", files={"sequence": ("s.y4m", sequence, "video/x-raw")}
    )
    assert response.status_code == 200
    report = response.json()

    assert report["width"] == 192
    assert report["height"] == 192
    assert report["qps"] == list(service.QPS)
    assert len(report["points"]) == len(service.QPS)

    for point in report["points"]:
        assert point["qp"] in service.QPS
        # Both rate axes are present, because the mechanism changes the luma
        # signalling and nothing else.
        assert point["explicit"]["bytes"] > 0
        assert point["inferred"]["bytes"] > 0
        # The distortion axis is present, because a rate with no distortion
        # is not a measurement.
        assert point["explicit"]["psnr_db"] > 0
        assert point["inferred"]["psnr_db"] > 0

    assert [row["margin"] for row in report["margin_sweep"]] == list(service.MARGINS)


def test_verify_is_a_status_code(client: TestClient, sequence: bytes) -> None:
    """`?verify=1` answers 200 for a complete run, as a script expects."""
    response = client.post(
        "/analyze",
        files={"sequence": ("s.y4m", sequence, "video/x-raw")},
        data={"verify": "1"},
    )
    assert response.status_code == 200
    assert response.json()["measured"] == "complete"


def test_the_html_report_is_self_contained(client: TestClient, sequence: bytes) -> None:
    """The HTML report has nothing to fetch, so it can be saved and read."""
    response = client.post(
        "/analyze",
        files={"sequence": ("s.y4m", sequence, "video/x-raw")},
        data={"format": "html"},
    )
    assert response.status_code == 200
    assert "explicit" in response.text
    assert "margin" in response.text


def test_the_two_header_spellings_are_both_read() -> None:
    """A parser that knows one Y4M header form rejects a sequence the codec reads."""
    assert service._y4m_dimensions(b"YUV4MPEG2 W192 H192 F25:1 Ip\nFRAME\n") == (192, 192)
    assert service._y4m_dimensions(b"YUV4MPEG2 W352H288 F25:1 Ip\nFRAME\n") == (352, 288)
    assert service._y4m_dimensions(b"YUV4MPEG2 C420jpeg W64 H64\nFRAME\n") == (64, 64)


def test_a_non_y4m_body_has_no_dimensions() -> None:
    """A body that is not a sequence has no dimensions rather than a guess."""
    assert service._y4m_dimensions(b"not a y4m") is None
    assert service._y4m_dimensions(b"YUV4MPEG2 F25:1\nFRAME\n") is None


def test_the_frame_count_is_counted_from_the_headers() -> None:
    """The frame count comes from the file, not from the caller."""
    assert service._count_frames(b"FRAME\nFRAME\nFRAME\n") == 3
    assert service._count_frames(b"") == 0


def test_the_luma_plane_is_extracted_from_each_frame() -> None:
    """The luma plane is the first `width * height` bytes after a frame header."""
    body = b"YUV4MPEG2 W4 H4\nFRAME\n" + b"\x01" * 16 + b"\x02" * 4 + b"\x03" * 4
    plane = service._luma_of(body, 1)
    assert len(plane) == 16
    assert plane == b"\x01" * 16


def test_psnr_is_infinite_for_an_identical_plane() -> None:
    """An identical plane is reported as the ceiling, not as a division by zero."""
    assert service._psnr(b"\x80" * 64, b"\x80" * 64) == 99.0


def test_psnr_falls_as_the_error_grows() -> None:
    """A worse reconstruction is a lower PSNR, which is the whole point of it."""
    assert service._psnr(b"\x80" * 64, b"\x90" * 64) < service._psnr(b"\x80" * 64, b"\x81" * 64)


def test_psnr_of_differing_lengths_is_zero() -> None:
    """Two planes of different lengths are not a comparison."""
    assert service._psnr(b"\x80" * 64, b"\x80" * 32) == 0.0


def test_the_temporary_directory_is_removed(sequence: bytes) -> None:
    """A request leaves nothing behind: the temporary directory is request-scoped."""
    before = {path.name for path in service.Path(service.os.environ.get("TMPDIR", "/tmp")).glob("girc-analyse-*")}
    service.analyse(sequence)
    after = {path.name for path in service.Path(service.os.environ.get("TMPDIR", "/tmp")).glob("girc-analyse-*")}
    assert after == before


def test_the_report_serialises_to_json(client: TestClient, sequence: bytes) -> None:
    """The report is JSON, so a script can assert on it without parsing text."""
    report = client.post(
        "/analyze", files={"sequence": ("s.y4m", sequence, "video/x-raw")}
    ).json()
    assert json.loads(json.dumps(report)) == report
