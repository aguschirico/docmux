#!/usr/bin/env python3
"""Add arXiv papers to manifest.tsv: download each source, find its main
file and take the short name from the arXiv title.

    python3 scripts/corpus/add.py 1406.2661 hep-th/9711200 ...

Papers without LaTeX source (PDF only, or a LaTeX file that only includes a
PDF) are reported and left out.
"""
import re
import subprocess
import sys
import time
import xml.etree.ElementTree as ET
from pathlib import Path

from fetch import download, fetch, main_file

HERE = Path(__file__).resolve().parent
MANIFEST = HERE / "manifest.tsv"
ATOM = "{http://www.w3.org/2005/Atom}"


def titles(ids: list[str]) -> dict[str, str]:
    out = {}
    for start in range(0, len(ids), 100):
        batch = ids[start : start + 100]
        url = "https://export.arxiv.org/api/query?max_results=100&id_list=" + ",".join(batch)
        feed = ET.fromstring(download(url))
        for entry in feed.iter(ATOM + "entry"):
            link = entry.findtext(ATOM + "id") or ""
            pid = re.sub(r"v\d+$", "", link.split("/abs/")[-1])
            out[pid] = " ".join((entry.findtext(ATOM + "title") or "").split())
        time.sleep(3)  # arXiv asks bulk clients for one request every 3 seconds
    return out


def short(title: str) -> str:
    """The acronym before a colon ("Adam: A Method..." -> "Adam"), or the
    first words of the title."""
    head = title.split(":")[0].strip()
    if len(head) <= 24:
        return head
    return " ".join(title.split()[:3])[:24]


def main() -> int:
    known = {line.split("\t")[0] for line in MANIFEST.read_text().splitlines() if line and not line.startswith("#")}
    ids = [i for i in sys.argv[1:] if i not in known]
    if not ids:
        return 0
    names = titles(ids)
    for pid in ids:
        title = names.get(pid)
        if not title:
            print(f"skip {pid}: not found on arXiv")
            continue
        try:
            has_source = fetch(pid)
        except subprocess.CalledProcessError as err:
            print(f"skip {pid}: download failed ({err.stderr.decode().strip()})")
            continue
        finally:
            time.sleep(3)  # arXiv asks bulk clients for one request every 3 seconds
        main_tex = main_file(pid) if has_source else None
        if not main_tex:
            print(f"skip {pid}: no LaTeX source, or it only includes a PDF")
            continue
        with MANIFEST.open("a") as f:
            f.write(f"{pid}\t{main_tex}\t{short(title)}\n")
        print(f"add  {pid}  {main_tex}  {title}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
