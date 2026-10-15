#!/usr/bin/env python3
"""Convert the corpus with docmux and report, for every paper:

- Typst: compile errors and raw LaTeX commands left in the output.
- Word: the .docx read back with pandoc, which parses Word's native equations
  (OMML) independently: equations found, and LaTeX commands left in the text.

Run fetch.py first, then:

    python3 scripts/corpus/check.py [--pandoc] [--no-docx] [arxiv-id ...]

Images are swapped for boxes before compiling: EPS/PDF figures are expected
to be missing and would otherwise stop Typst before it reports real errors.
Exits non-zero if any paper has errors or leaks.
"""
import json
import os
import re
import subprocess
import sys
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass, field
from pathlib import Path

from fetch import source_dir

HERE = Path(__file__).resolve().parent
OUT = HERE / ".cache" / "out"
DOCMUX = HERE.parent.parent / "target" / "release" / "docmux"

IMAGE = re.compile(r'image\("(?:[^"\\]|\\.)*"(?:, alt: "(?:[^"\\]|\\.)*")?(?:, width: [^,)]*)?\)')
# Code, raw text and string literals; a `\"` is an escaped quote, not a string.
SKIP = re.compile(r'```.*?```|`[^`\n]*`|#raw\("(?:[^"\\]|\\.)*"\)|(?<!\\)"(?:[^"\\\n]|\\.)*"', re.S)
# `\n` is not a LaTeX command: papers write it as the newline escape ("{\textbackslash}n").
CMD = re.compile(r"\\(?!n\b)[a-zA-Z]+")
# A backslash the source prints on purpose: `{\textbackslash}name`, `$\backslash$name`.
LITERAL = re.compile(r"(?:\{\\textbackslash\}|\\textbackslash(?:\{\})?\s*|\$\\backslash\$)([a-zA-Z]+)")


def literal_commands(pid: str) -> set[str]:
    """Command names the paper prints as text, so LaTeX shows them too."""
    names: set[str] = set()
    for path in source_dir(pid).rglob("*"):
        if path.suffix in (".tex", ".bbl"):
            names.update("\\" + m for m in LITERAL.findall(path.read_text(errors="replace")))
    return names


def without_literals(leaks: Counter[str], pid: str) -> Counter[str]:
    if not leaks:
        return leaks
    literal = literal_commands(pid)
    return Counter({k: v for k, v in leaks.items() if k not in literal})


@dataclass
class Result:
    errors: int = 0
    leaks: Counter[str] = field(default_factory=Counter)
    first: str = ""
    equations: int = 0

    @property
    def ok(self) -> bool:
        return self.errors == 0 and not self.leaks


def manifest() -> list[tuple[str, str, str]]:
    rows = []
    for line in (HERE / "manifest.tsv").read_text().splitlines():
        if line and not line.startswith("#"):
            pid, main, name = line.split("\t")
            rows.append((pid, main, name))
    return rows


def run(cmd: list[str], cwd: Path) -> subprocess.CompletedProcess[str]:
    try:
        return subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, timeout=120)
    except subprocess.TimeoutExpired:
        return subprocess.CompletedProcess(cmd, 124, "", "timeout")


def output(tool: str, pid: str, ext: str) -> Path:
    return OUT / f"{pid.replace('/', '_')}.{tool}.{ext}"


def check_typst(tool: str, pid: str, main: str) -> Result:
    typ = output(tool, pid, "typ")
    if tool == "docmux":
        cmd = [str(DOCMUX), main, "-t", "typst", "-o", str(typ)]
    else:
        cmd = ["pandoc", main, "-f", "latex", "-t", "typst", "-o", str(typ)]
    conv = run(cmd, source_dir(pid))
    if conv.returncode or not typ.exists():
        return Result(errors=-1, first="conversion failed: " + conv.stderr.strip()[:200])
    source = typ.read_text()
    probe = output(tool, pid, "probe.typ")
    probe.write_text(IMAGE.sub("rect(width: 1cm)", source))
    res = run(["typst", "compile", "--root", "/", str(probe), str(output(tool, pid, "pdf"))], OUT)
    errors = [l for l in res.stderr.splitlines() if l.startswith("error")]
    if res.returncode == 124:
        errors.append("error: typst timed out")
    leaks = without_literals(Counter(CMD.findall(SKIP.sub("", source))), pid)
    return Result(len(errors), leaks, errors[0] if errors else "")


def walk(node: object, math: list[int], text: list[str]) -> None:
    """Counts Math nodes and collects Str text from a pandoc JSON AST."""
    if isinstance(node, dict):
        if node.get("t") == "Math":
            math[0] += 1
            return
        if node.get("t") == "Str":
            text.append(node["c"])
            return
        for value in node.values():
            walk(value, math, text)
    elif isinstance(node, list):
        for value in node:
            walk(value, math, text)


def check_docx(pid: str, main: str) -> Result:
    docx = output("docmux", pid, "docx")
    conv = run([str(DOCMUX), main, "-t", "docx", "-o", str(docx)], source_dir(pid))
    if conv.returncode or not docx.exists():
        return Result(errors=-1, first="conversion failed: " + conv.stderr.strip()[:200])
    back = run(["pandoc", str(docx), "-f", "docx", "-t", "json"], OUT)
    if back.returncode:
        return Result(errors=1, first="pandoc could not read the .docx: " + back.stderr.strip()[:200])
    math, text = [0], []
    walk(json.loads(back.stdout)["blocks"], math, text)
    leaks = without_literals(Counter(CMD.findall(" ".join(text))), pid)
    return Result(0, leaks, "", math[0])


def describe(label: str, r: Result) -> str:
    if r.errors < 0:
        return f"{label}: {r.first}"
    detail = f"{label}: {r.errors} errors, {sum(r.leaks.values())} leaks"
    if r.errors:
        detail += f" ({r.first})"
    return detail


def main() -> int:
    args = sys.argv[1:]
    with_pandoc = "--pandoc" in args
    with_docx = "--no-docx" not in args
    only = [a for a in args if not a.startswith("--")]
    OUT.mkdir(parents=True, exist_ok=True)
    rows = [r for r in manifest() if not only or r[0] in only]

    def one(row: tuple[str, str, str]) -> tuple[Result, Result | None, Result | None]:
        pid, main_file, _ = row
        typst = check_typst("docmux", pid, main_file)
        docx = check_docx(pid, main_file) if with_docx else None
        ref = check_typst("pandoc", pid, main_file) if with_pandoc else None
        return typst, docx, ref

    with ThreadPoolExecutor(max_workers=os.cpu_count()) as pool:
        results = list(pool.map(one, rows))

    typst_ok = docx_ok = equations = 0
    for (pid, _, name), (typst, docx, ref) in zip(rows, results):
        ok = typst.ok and (docx is None or docx.ok)
        typst_ok += typst.ok
        parts = [describe("typst", typst)]
        if docx:
            docx_ok += docx.ok
            equations += docx.equations
            parts.append(describe("docx", docx) + f", {docx.equations} equations")
        if ref:
            parts.append(describe("pandoc typst", ref))
        print(f"{'ok ' if ok else 'FAIL'} {pid:16} {name[:24]:24} " + " | ".join(parts))
        for label, r in (("typst", typst), ("docx", docx)):
            if r and r.leaks:
                print(f"     {label} leaks: {r.leaks.most_common(8)}")

    print(f"\n{len(rows)} papers. Typst compiles cleanly: {typst_ok}/{len(rows)}.", end="")
    if with_docx:
        print(f" Word without leftover LaTeX: {docx_ok}/{len(rows)}, {equations} native equations.", end="")
    print()
    failed = typst_ok < len(rows) or (with_docx and docx_ok < len(rows))
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
