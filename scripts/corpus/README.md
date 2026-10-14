# Real-document corpus

A regression check against real arXiv papers, not just unit fixtures.

```sh
cargo build --release -p docmux-cli
scripts/corpus/fetch.sh            # downloads sources into scripts/corpus/.cache
python3 scripts/corpus/check.py    # add --pandoc for a side-by-side column
```

For every paper in `manifest.tsv` it converts LaTeX to Typst, compiles the
result with `typst`, and counts LaTeX commands left in the output. The bar is
0 errors and 0 leaks. Sources are not committed (arXiv licenses vary).

When a paper exposes a bug, fix it with a minimal, self-written test in the
crate that owns the code, and keep the paper in the manifest.
