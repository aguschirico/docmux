#!/usr/bin/env python3
"""Convert the corpus with docmux and report Typst compile errors and raw
LaTeX left in the output. Run ./fetch.sh first, then:

    python3 scripts/corpus/check.py [--pandoc] [arxiv-id ...]

Images are swapped for boxes before compiling: EPS/PDF figures are expected
to be missing and would otherwise stop Typst before it reports real errors.
Exits non-zero if any paper has errors or leaks.
"""
import re
import subprocess
import sys
from collections import Counter
from pathlib import Path

HERE = Path(__file__).resolve().parent
CACHE = HERE / ".cache"
OUT = CACHE / "out"
DOCMUX = HERE.parent.parent / "target" / "release" / "docmux"

IMAGE = re.compile(r'image\("(?:[^"\\]|\\.)*"(?:, alt: "(?:[^"\\]|\\.)*")?(?:, width: [^,)]*)?\)')
SKIP = re.compile(r'```.*?```|`[^`\n]*`|#raw\("(?:[^"\\]|\\.)*"\)|"(?:[^"\\\n]|\\.)*"', re.S)
CMD = re.compile(r"\\[a-zA-Z]+")


def manifest() -> list[tuple[str, str, str]]:
    rows = []
    for line in (HERE / "manifest.tsv").read_text().splitlines():
        if line and not line.startswith("#"):
            pid, main, name = line.split("\t")
            rows.append((pid, main, name))
    return rows


def run(cmd: list[str], cwd: Path) -> subprocess.CompletedProcess[str]:
    try:
        return subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, timeout=60)
    except subprocess.TimeoutExpired:
        return subprocess.CompletedProcess(cmd, 124, "", "timeout")


def check(tool: str, pid: str, main: str) -> tuple[int, Counter[str], str]:
    typ = OUT / f"{pid}.{tool}.typ"
    if tool == "docmux":
        cmd = [str(DOCMUX), main, "-t", "typst", "-o", str(typ)]
    else:
        cmd = ["pandoc", main, "-f", "latex", "-t", "typst", "-o", str(typ)]
    conv = run(cmd, CACHE / pid)
    if conv.returncode or not typ.exists():
        return -1, Counter(), conv.stderr.strip()[:100]
    source = typ.read_text()
    probe = typ.with_suffix(".probe.typ")
    probe.write_text(IMAGE.sub("rect(width: 1cm)", source))
    res = run(["typst", "compile", "--root", "/", str(probe), str(typ.with_suffix(".pdf"))], OUT)
    errors = [l for l in res.stderr.splitlines() if l.startswith("error")]
    leaks = Counter(CMD.findall(SKIP.sub("", source)))
    return len(errors), leaks, errors[0] if errors else ""


def main() -> int:
    args = sys.argv[1:]
    tools = ["docmux"] + (["pandoc"] if "--pandoc" in args else [])
    only = [a for a in args if not a.startswith("--")]
    OUT.mkdir(parents=True, exist_ok=True)
    failed = False
    for pid, main_file, name in manifest():
        if only and pid not in only:
            continue
        for tool in tools:
            errors, leaks, first = check(tool, pid, main_file)
            ok = errors == 0 and not leaks
            failed |= tool == "docmux" and not ok
            status = "ok " if ok else "FAIL"
            detail = "conversion failed: " + first if errors < 0 else f"{errors} errors, {sum(leaks.values())} leaks"
            print(f"{status} {tool:7} {pid:11} {name:12} {detail} {first if errors > 0 else ''}")
            if leaks:
                print(f"     leaks: {leaks.most_common(8)}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
