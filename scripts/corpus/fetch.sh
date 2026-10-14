#!/usr/bin/env bash
# Download the LaTeX sources listed in manifest.tsv into .cache/ (gitignored).
# arXiv licenses vary, so the sources are never committed.
set -euo pipefail
cd "$(dirname "$0")"
mkdir -p .cache
grep -v '^#' manifest.tsv | while IFS=$'\t' read -r id _main _name; do
    dir=".cache/$id"
    [ -d "$dir" ] && continue
    mkdir -p "$dir"
    curl -sSL -A "docmux-corpus" -o ".cache/$id.tar.gz" "https://arxiv.org/e-print/$id"
    tar -xzf ".cache/$id.tar.gz" -C "$dir"
    echo "fetched $id"
done
