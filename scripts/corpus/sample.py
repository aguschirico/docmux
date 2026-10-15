#!/usr/bin/env python3
"""Print random arXiv ids, to feed add.py (which drops ids that don't exist):

    python3 scripts/corpus/add.py $(python3 scripts/corpus/sample.py --count 240 --seed 1)

Famous papers tend to have tidy LaTeX. A random sample, across fields and
across arXiv's whole history, finds what they don't. Ids are drawn directly:
old-style ones (hep-th/9711200) from 1995 to 2006 across the original
archives, new-style ones (2106.09685) from 2007 on, where the mix of fields
follows what people actually submit.
"""
import argparse
import random

OLD_ARCHIVES = [
    "math", "hep-th", "hep-ph", "hep-ex", "astro-ph", "cond-mat", "quant-ph",
    "gr-qc", "cs", "physics", "nlin", "q-bio", "nucl-th",
]


def old_style(rng: random.Random) -> str:
    year = rng.randrange(1995, 2007)
    month = rng.randrange(1, 13)
    return f"{rng.choice(OLD_ARCHIVES)}/{year % 100:02}{month:02}{rng.randrange(1, 400):03}"


def new_style(rng: random.Random) -> str:
    year = rng.randrange(2008, 2026)
    month = rng.randrange(1, 13)
    # Monthly submissions grew from about 5,000 to more than 20,000.
    cap = 4000 + (year - 2008) * 1000
    number = rng.randrange(1, cap)
    width = 4 if year < 2015 else 5
    return f"{year % 100:02}{month:02}.{number:0{width}}"


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--count", type=int, default=240)
    parser.add_argument("--seed", type=int, default=1)
    args = parser.parse_args()
    rng = random.Random(args.seed)
    ids = {old_style(rng) if rng.random() < 0.3 else new_style(rng) for _ in range(args.count)}
    print(" ".join(sorted(ids)))


if __name__ == "__main__":
    main()
