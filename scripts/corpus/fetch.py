#!/usr/bin/env python3
"""Download the LaTeX sources listed in manifest.tsv into .cache/ (gitignored).
arXiv licenses vary, so the sources are never committed.

    python3 scripts/corpus/fetch.py
"""
import gzip
import io
import re
import subprocess
import sys
import tarfile
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
CACHE = HERE / ".cache"


def source_dir(pid: str) -> Path:
    """Old-style ids (hep-th/9711200) contain a slash."""
    return CACHE / pid.replace("/", "_")


def download(url: str) -> bytes:
    return subprocess.run(
        ["curl", "-sSfL", "-A", "docmux-corpus", "--max-time", "120", url],
        capture_output=True, check=True,
    ).stdout


def fetch(pid: str) -> bool:
    """Download and unpack one paper. arXiv serves a tarball, a single gzipped
    .tex file, or a PDF when there is no source. Returns False for a PDF."""
    out = source_dir(pid)
    if out.is_dir():
        return any(out.rglob("*.tex"))
    data = download(f"https://arxiv.org/e-print/{pid}")
    if data.startswith(b"%PDF"):
        return False
    raw = gzip.decompress(data) if data[:2] == b"\x1f\x8b" else data
    out.mkdir(parents=True)
    try:
        with tarfile.open(fileobj=io.BytesIO(raw)) as tar:
            # Plain files and folders only, all inside `out`.
            safe = [
                m for m in tar.getmembers()
                if (m.isfile() or m.isdir()) and not m.name.startswith("/") and ".." not in Path(m.name).parts
            ]
            tar.extractall(out, members=safe)
    except tarfile.ReadError:
        (out / "main.tex").write_bytes(raw)
    return any(out.rglob("*.tex"))


DOCCLASS = re.compile(rb"^[ \t]*\\documentclass", re.M)
INCLUDEPDF = re.compile(rb"^[ \t]*\\includepdf", re.M)


def main_file(pid: str) -> str | None:
    """The .tex file with \\documentclass and \\begin{document}; if several
    have both, the largest. None when the paper is only an embedded PDF."""
    root = source_dir(pid)
    candidates = []
    for tex in root.rglob("*.tex"):
        text = tex.read_bytes()
        if DOCCLASS.search(text) and b"\\begin{document}" in text:
            candidates.append((len(text), tex))
    if not candidates:
        return None
    tex = max(candidates)[1]
    if INCLUDEPDF.search(tex.read_bytes()):
        return None
    return str(tex.relative_to(root))


def main() -> int:
    for line in (HERE / "manifest.tsv").read_text().splitlines():
        if not line or line.startswith("#"):
            continue
        pid = line.split("\t")[0]
        if source_dir(pid).is_dir():
            continue
        print(f"fetch {pid}", "ok" if fetch(pid) else "no source")
        time.sleep(3)  # arXiv asks bulk clients for one request every 3 seconds
    return 0


if __name__ == "__main__":
    sys.exit(main())
