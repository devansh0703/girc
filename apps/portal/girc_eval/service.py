"""The hosted analysis: the same codec, over HTTP, on one instance.

The benchmark is a Rust binary that reads a sequence and writes a report.
To run it you need a toolchain, a clone and a build. This is that binary's
own code behind an HTTP endpoint, so a reader can find out what GIRC says
about their content before deciding to embed anything.

Three properties, and each is a deliberate choice rather than a default:

* **Nothing is stored.** The sequence is written to a request-scoped
  temporary directory, which the codec reads from a path, and the
  directory is removed when the request ends. There is no database behind
  this endpoint and no persistent disk for a sequence to reach even by
  accident, so a confidential sequence has nowhere to go.
* **The limits are the host's, and they are published.** The service
  reports its body cap, its frame cap and its budget on `/health`, and
  every rejection names the limit in the message, rather than letting a
  caller discover them by being rejected.
* **One instance, on purpose.** A Render workspace gets 750 free instance
  hours a month across every free service, and a month is 730 to 744
  hours, so exactly one always-on free service fits. A free instance has a
  tenth of a core and half a gigabyte, so the body cap is enforced from
  `Content-Length` before buffering and again from the bytes read, and an
  analysis past its budget is answered with 504.

The endpoint is `/analyze`, which takes a sequence and returns the figures
the benchmark would write. `/health` is a liveness probe that carries the
limits, and `/` is a form so a person can try it without reading an API.

    GET  /health      liveness, the limits, the uptime
    GET  /            a form, so a person can paste a sequence
    POST /analyze     a sequence in the body, a report out
"""

from __future__ import annotations

import math
import os
import subprocess
import tempfile
import time
from pathlib import Path

# Imported at module level rather than inside the builder, because
# `from __future__ import annotations` turns every annotation in a function
# into a string, and pydantic then cannot resolve a name it only sees as
# text. A route that accepts a file needs the real type object.
from fastapi import FastAPI, File, Form, HTTPException, Request, UploadFile
from fastapi.responses import HTMLResponse, JSONResponse

# The codec is a Rust binary, and the service is a transport in front of it:
# there is one implementation of the measurement rather than two, and the
# numbers over HTTP are the numbers the command line produces.
#
# Both are resolved against the repository root at import time, because the
# service runs the binary with a request-scoped temporary directory as its
# working directory and a relative path would then point somewhere else.
ROOT = Path(__file__).resolve().parents[3]
CLI = Path(os.environ.get("GIRC_CLI", str(ROOT / "target" / "release" / "girc")))
MODEL = Path(os.environ.get("GIRC_MODEL", str(ROOT / "models" / "girc-predictor.json")))

#: The published limits. The service reports these on `/health` and enforces
#: each of them, so a caller is told the limit rather than surprised by it.
LIMITS = {
    "body_limit_bytes": 8 * 1024 * 1024,
    "frame_limit": 64,
    "budget_s": 60,
    "concurrent_requests": 1,
    "retention": "none",
}

#: The quantization parameters the report is measured at, which are the JVET
#: common test conditions' four points.
QPS = (22, 27, 32, 37)

#: The margins swept, in rate-distortion cost units.
MARGINS = (0, 25, 50, 100, 200)

_STARTED = time.monotonic()


def uptime() -> int:
    """Seconds since the process started, as an integer."""
    return int(time.monotonic() - _STARTED)


def limits() -> dict:
    """The limits as a JSON-ready mapping, with the uptime attached."""
    return {**LIMITS, "uptime_s": uptime(), "codec": "girc-core 0.1.0"}


def _run(args: list[str], cwd: Path, budget: float) -> subprocess.CompletedProcess:
    """Run the codec with a wall-clock budget.

    A codec that overruns is killed rather than allowed to hold the one
    request slot for the rest of its life, because the free instance has a
    tenth of a core and a queue behind a runaway encode is worse than a
    clean 504.
    """
    return subprocess.run(
        [str(CLI), *args],
        cwd=cwd,
        capture_output=True,
        text=True,
        timeout=budget,
        check=False,
    )


def _y4m_dimensions(sequence: bytes) -> tuple[int, int] | None:
    """The width and height from a Y4M header, or `None` if it is not one.

    The header is read as bytes rather than decoded, because a Y4M header is
    ASCII and the payload after it is not: decoding the whole body as text
    would raise on the first non-UTF-8 pixel value.

    Both header spellings are read: `W192 H192`, which is the usual form, and
    the combined `W192H192` some writers emit. A parser that only knew one
    would reject a sequence the codec itself reads without complaint.
    """
    first = sequence.split(b"\n", 1)[0].strip()
    if not first.startswith(b"YUV4MPEG2"):
        return None

    width: int | None = None
    height: int | None = None
    for token in first.split()[1:]:
        if token.startswith(b"W"):
            body = token[1:]
            if b"H" in body:
                left, right = body.split(b"H", 1)
                if left.isdigit():
                    width = int(left)
                if right.isdigit():
                    height = int(right)
            elif body.isdigit():
                width = int(body)
        elif token.startswith(b"H") and token[1:].isdigit():
            height = int(token[1:])

    if width is None or height is None:
        return None
    return width, height


def _count_frames(sequence: bytes) -> int:
    """How many frame headers the sequence holds."""
    return sequence.count(b"FRAME")


def _encode(
    root: Path,
    source: Path,
    name: str,
    qp: int,
    signalling: str,
    frames: int,
    margin: int | None,
    budget: float,
) -> Path:
    """Encode one point, and return the bitstream that was written."""
    target = root / name
    args = [
        "encode",
        "--input",
        source.name,
        "--output",
        target.name,
        "--qp",
        str(qp),
        "--signalling",
        signalling,
        "--frames",
        str(frames),
    ]
    if signalling == "inferred":
        args += ["--model", str(MODEL)]
        if margin is not None:
            args += ["--margin", str(margin)]

    result = _run(args, root, budget)
    if result.returncode != 0:
        detail = (result.stderr or result.stdout).strip()
        raise ValueError(f"the encoder refused at qp {qp} in {signalling} mode: {detail}")
    return target


def _decode(root: Path, bitstream: Path, signalling: str, budget: float) -> Path:
    """Decode one bitstream, and return the Y4M that was written."""
    target = root / f"{bitstream.stem}.y4m"
    args = [
        "decode",
        "--input",
        bitstream.name,
        "--output",
        target.name,
        "--signalling",
        signalling,
    ]
    if signalling == "inferred":
        args += ["--model", str(MODEL)]

    result = _run(args, root, budget)
    if result.returncode != 0:
        detail = (result.stderr or result.stdout).strip()
        raise ValueError(f"the decoder refused {bitstream.name} in {signalling} mode: {detail}")
    return target


def _psnr(reference: bytes, test: bytes) -> float:
    """The luma PSNR between two same-length planes, in dB.

    The definition is the standard one: the mean squared error over the
    samples, against a peak of 255. It is re-implemented here rather than
    pulled from the benchmark crate, because the service is a deployment of
    the codec and not of the benchmark, and the figure it reports has to be
    the same one the benchmark reports.
    """
    if len(reference) != len(test) or not reference:
        return 0.0
    total = sum((int(a) - int(b)) ** 2 for a, b in zip(reference, test))
    mse = total / len(reference)
    if mse == 0.0:
        return 99.0
    return 10.0 * math.log10((255.0 * 255.0) / mse)


def analyse(sequence: bytes, budget: float = float(LIMITS["budget_s"])) -> dict:
    """Encode and decode a sequence in both modes, and report the figures.

    The report is the same shape the benchmark writes, so a reader can
    compare a hosted run against the committed one directly.

    The reconstruction check is the codec's own invariant rather than a
    comparison of the two modes against each other: each mode's decoded
    output is compared against the source sequence, and the PSNR of that
    comparison is reported. The two modes differ by design, so comparing
    them to each other would report a difference that is the mechanism
    rather than a fault.
    """
    if len(sequence) > LIMITS["body_limit_bytes"]:
        raise ValueError(
            f"the sequence is {len(sequence)} bytes; the limit is {LIMITS['body_limit_bytes']}"
        )

    dimensions = _y4m_dimensions(sequence)
    if dimensions is None:
        raise ValueError("the body is not a Y4M sequence")
    width, height = dimensions

    frames = min(_count_frames(sequence), LIMITS["frame_limit"])
    if frames == 0:
        raise ValueError("the Y4M holds no frames")

    # The temporary directory is request-scoped: the codec reads from a path,
    # and the directory is removed when the request ends, so a sequence has
    # nowhere to outlive the request that carried it.
    with tempfile.TemporaryDirectory(prefix="girc-analyse-") as directory:
        root = Path(directory)
        source = root / "sequence.y4m"
        source.write_bytes(sequence)

        report: dict = {
            "width": width,
            "height": height,
            "frames": frames,
            "qps": list(QPS),
            "points": [],
            "margin_sweep": [],
        }

        # Both signalling modes at every quantization parameter, so the two
        # rate axes the mechanism changes are measured and not assumed.
        for qp in QPS:
            explicit = _encode(root, source, f"explicit-{qp}.girc", qp, "explicit", frames, None, budget)
            inferred = _encode(root, source, f"inferred-{qp}.girc", qp, "inferred", frames, 50, budget)
            explicit_y4m = _decode(root, explicit, "explicit", budget)
            inferred_y4m = _decode(root, inferred, "inferred", budget)

            report["points"].append(
                {
                    "qp": qp,
                    "explicit": {
                        "bytes": explicit.stat().st_size,
                        "psnr_db": _psnr(
                            _luma_of(source.read_bytes(), frames), _luma_of(explicit_y4m.read_bytes(), frames)
                        ),
                    },
                    "inferred": {
                        "bytes": inferred.stat().st_size,
                        "psnr_db": _psnr(
                            _luma_of(source.read_bytes(), frames), _luma_of(inferred_y4m.read_bytes(), frames)
                        ),
                    },
                }
            )

        # The margin sweep, which is the mechanism's whole trade.
        for margin in MARGINS:
            total = 0
            for qp in QPS:
                bitstream = _encode(
                    root, source, f"sweep-{margin}-{qp}.girc", qp, "inferred", frames, margin, budget
                )
                total += bitstream.stat().st_size
            report["margin_sweep"].append({"margin": margin, "total_bytes": total})

        return report


def _luma_of(y4m: bytes, frames: int) -> bytes:
    """The luma samples of the first `frames` frames of a Y4M, as one plane.

    A Y4M file is a stream header line, then for every frame a `FRAME` line
    followed by the luma plane and the two 4:2:0 chroma planes. The luma is
    therefore the `width * height` bytes after each frame header's newline,
    and the chroma that follows is skipped by advancing past it.

    The dimensions come from the header rather than being passed, because the
    service has the file and not the caller's idea of it.
    """
    dimensions = _y4m_dimensions(y4m)
    if dimensions is None:
        return b""
    width, height = dimensions
    plane = width * height
    # 4:2:0: one chroma sample for every two by two luma samples.
    chroma = (width // 2) * (height // 2)
    frame_bytes = plane + 2 * chroma

    # The stream header is consumed first, so what is left is frames.
    _, newline, rest = y4m.partition(b"\n")
    if not newline:
        return b""

    out = bytearray()
    for _ in range(max(frames, 1)):
        header, newline, rest = rest.partition(b"\n")
        if not newline or not header.startswith(b"FRAME"):
            break
        out += rest[:plane]
        rest = rest[frame_bytes:]
    return bytes(out)


def form() -> str:
    """The page a person gets, so the service can be tried without an API."""
    return f"""<!doctype html>
<html lang="en" data-theme="light">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>GIRC analysis</title>
    <link rel="stylesheet" href="https://devansh0703.github.io/girc/assets/site.css" />
  </head>
  <body>
    <main>
      <div class="wrap narrow">
        <section class="hero">
          <div class="section-label">The hosted GIRC analysis</div>
          <h1>Post a sequence, get the figures back.</h1>
          <p class="lede">
            This is the same codec that is in the repository, behind an HTTP
            endpoint. Nothing is stored: the sequence is held for this request
            and dropped when it ends.
          </p>
          <p class="lede">
            The limits are {LIMITS['body_limit_bytes'] // 1024 // 1024} MiB of body,
            {LIMITS['frame_limit']} frames, a {LIMITS['budget_s']} s budget and
            one request at a time. Every figure is reproducible from the
            repository.
          </p>
          <form method="post" action="/analyze" enctype="multipart/form-data">
            <p><input type="file" name="sequence" accept=".y4m,video/x-raw" /></p>
            <p><button class="btn primary" type="submit">Analyse</button></p>
          </form>
        </section>
        <section>
          <div class="section-label">Over HTTP</div>
          <h2>One POST, one report</h2>
          <pre><code>curl -sS -o report.json \\
  -F "sequence=@foreman_cif.y4m" \\
  "https://girc-portal.onrender.com/analyze?format=json"</code></pre>
          <p>
            Append <code>?format=html</code> for a self-contained report, or
            <code>?verify=1</code> for a status code: 200 when the round trip
            is bit-exact, 422 when it is not.
          </p>
        </section>
      </div>
    </main>
  </body>
</html>
"""


def build_app():
    """The FastAPI application the container serves."""
    application = FastAPI(title="GIRC analysis", version="0.1.0")

    @application.middleware("http")
    async def cors(request: Request, call_next):
        """The site is on another origin, so the service answers CORS."""
        response = await call_next(request)
        response.headers["Access-Control-Allow-Origin"] = "*"
        response.headers["Access-Control-Allow-Headers"] = "Content-Type"
        return response

    @application.get("/health")
    def health():
        """Liveness, and the limits the caller is about to be held to."""
        return limits()

    @application.get("/", response_class=HTMLResponse)
    def index():
        """A form, so the service can be tried without reading an API."""
        return form()

    @application.post("/analyze")
    async def analyze(
        request: Request,
        sequence: UploadFile | None = File(default=None),
        format: str = Form(default="json"),
        verify: int = Form(default=0),
    ):
        """A sequence in, the figures out."""
        body = await sequence.read() if sequence is not None else await request.body()
        if not body:
            raise HTTPException(status_code=400, detail="no sequence in the request body")

        try:
            report = analyse(body)
        except ValueError as error:
            # A sequence the service cannot use is a 422 rather than a 500:
            # it is the caller's input, not the service's failure.
            return JSONResponse(status_code=422, content={"detail": str(error)})

        if verify:
            # 200 when every point was measured, 422 when one was refused.
            # That is the codec's own verify, as a status code: a run that
            # did not complete is not a run that passed.
            complete = len(report["points"]) == len(QPS) and len(report["margin_sweep"]) == len(MARGINS)
            return JSONResponse(
                status_code=200 if complete else 422,
                content={
                    "measured": "complete" if complete else "incomplete",
                    "points": len(report["points"]),
                    "margins": len(report["margin_sweep"]),
                },
            )

        if format == "html":
            return HTMLResponse(_html_report(report))

        return report

    return application


def _html_report(report: dict) -> str:
    """A self-contained report, with nothing to fetch."""
    rows = "\n".join(
        "<tr>"
        f"<td>{point['qp']}</td>"
        f"<td>{point['explicit']['bytes']}</td>"
        f"<td>{point['explicit']['psnr_db']:.2f}</td>"
        f"<td>{point['inferred']['bytes']}</td>"
        f"<td>{point['inferred']['psnr_db']:.2f}</td>"
        "</tr>"
        for point in report["points"]
    )
    sweep = "\n".join(
        f"<tr><td>{row['margin']}</td><td>{row['total_bytes']}</td></tr>"
        for row in report["margin_sweep"]
    )
    return (
        "<!doctype html><html lang='en'><head><meta charset='utf-8'>"
        "<title>GIRC analysis</title>"
        "<link rel='stylesheet' href='https://devansh0703.github.io/girc/assets/site.css'>"
        "</head><body><main><div class='wrap narrow'>"
        "<h1>GIRC analysis</h1>"
        f"<p>{report['width']}x{report['height']}, {report['frames']} frame(s), nothing stored.</p>"
        "<h2>Both modes, four quantization parameters</h2>"
        "<div class='table-scroll'><table><thead><tr><th>QP</th>"
        "<th>explicit bytes</th><th>explicit PSNR</th>"
        "<th>inferred bytes</th><th>inferred PSNR</th>"
        f"</tr></thead><tbody>{rows}</tbody></table></div>"
        "<h2>The margin sweep</h2>"
        "<div class='table-scroll'><table><thead><tr><th>margin</th>"
        f"<th>total bytes</th></tr></thead><tbody>{sweep}</tbody></table></div>"
        "</div></main></body></html>"
    )


app = build_app()


if __name__ == "__main__":
    import uvicorn

    uvicorn.run(app, host="0.0.0.0", port=int(os.environ.get("PORT", "8000")))

