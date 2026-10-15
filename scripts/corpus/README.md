# Real-document corpus

A regression check against real arXiv papers, not just unit fixtures: 104
papers across machine learning, mathematics, physics, astronomy, statistics
and biology, written between 1998 and 2023.

```sh
cargo build --release -p docmux-cli
python3 scripts/corpus/fetch.py    # downloads sources into scripts/corpus/.cache
python3 scripts/corpus/check.py    # --pandoc adds pandoc's Typst, --no-docx skips Word
```

For every paper in `manifest.tsv` it checks two outputs:

- **Typst:** converts LaTeX to Typst, compiles the result with `typst`, and
  counts LaTeX commands left in the output.
- **Word:** converts LaTeX to `.docx` and reads it back with pandoc, which
  parses Word's native equations (OMML) on its own. It counts the equations
  pandoc finds and the LaTeX commands left in the text.

The bar is 0 errors and 0 leaks for every paper. A command the source itself
prints as text (`{\textbackslash}name`) is not a leak. Sources are not
committed (arXiv licenses vary).

To add papers:

```sh
python3 scripts/corpus/add.py 1406.2661 hep-th/9711200
```

It downloads each source, finds the main `.tex` file and takes the short name
from the arXiv title. Papers without LaTeX source (PDF only, plain TeX, or a
LaTeX file that only includes a PDF) are left out.

When a paper exposes a bug, fix it with a minimal, self-written test in the
crate that owns the code, and keep the paper in the manifest.
