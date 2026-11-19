#!/usr/bin/env python3
"""Generate a large synthetic book for the performance check.

    python scripts/gen-corpus.py OUT_DIR [--pages 230] [--image-mb 1.1] [--diagrams 120]

The corpus has 3 tiers, one hero image per page (about 1 MB each, so about 260 MB for 230
pages), headings, lists, code, a table per page, and optional Mermaid flowcharts. It is not run
on every push.
"""
import argparse
import os
import random
from pathlib import Path

from PIL import Image


def hero(path: Path, seed: int, target_bytes: int) -> None:
    rnd = random.Random(seed)
    w, h = 1800, 1200
    base = Image.linear_gradient("L").resize((w, h)).convert("RGB")
    tint = Image.new("RGB", (w, h), (rnd.randrange(256), rnd.randrange(256), rnd.randrange(256)))
    img = Image.blend(base, tint, 0.5)
    # Add noise until the JPEG reaches roughly the target size.
    amount = 0.04
    for _ in range(8):
        noise = Image.frombytes("RGB", (w, h), os.urandom(w * h * 3))
        out = Image.blend(img, noise, amount)
        out.save(path, "JPEG", quality=90)
        size = path.stat().st_size
        if size >= target_bytes * 0.9:
            break
        amount = min(0.6, amount * 1.8)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("out")
    ap.add_argument("--pages", type=int, default=230)
    ap.add_argument("--image-mb", type=float, default=1.1)
    ap.add_argument("--diagrams", type=int, default=120)
    args = ap.parse_args()

    out = Path(args.out)
    tiers = ["required", "additional", "bonus"]
    (out / "images").mkdir(parents=True, exist_ok=True)
    (out / "hideo.toml").write_text(
        'tiers = ["required", "additional", "bonus"]\n[book]\nlang = "en"\n'
        '[cover]\ntitle = "Corpus"\nauthor = "Generated"\n[diagrams]\nbackend = "off"\n',
        encoding="utf-8",
    )
    per_tier = [args.pages // 3 + (1 if i < args.pages % 3 else 0) for i in range(3)]
    diagrams_left = args.diagrams
    n = 0
    for tier, count in zip(tiers, per_tier):
        for i in range(1, count + 1):
            n += 1
            image = f"images/hero-{n:04d}.jpg"
            hero(out / image, n, int(args.image_mb * 1_000_000))
            body = [f"# Page {n}\n", "Introductory paragraph with *emphasis*, **strong** text and `code`. " * 6 + "\n"]
            for s in range(1, 5):
                body.append(f"## Section {n}.{s}\n")
                body.append("Some prose that fills the page with a realistic amount of text. " * 8 + "\n")
                body.append("- first point\n- second point\n  - nested point\n\n")
            body.append("```rust\nfn main() {\n    println!(\"page " + str(n) + "\");\n}\n```\n")
            body.append("| Name | Value | Notes |\n|------|------:|-------|\n" + "".join(
                f"| row {r} | {r * n} | some note {r} |\n" for r in range(1, 9)))
            if diagrams_left > 0:
                diagrams_left -= 1
                body.append(f"\n```mermaid \"Flow {n}\"\ngraph TD\n  A{n}[Start] --> B{n}{{Choice}}\n  B{n} -->|yes| C{n}[Do]\n  B{n} -->|no| D{n}[Skip]\n```\n")
            if n > 1:
                body.append(f"\nSee also [the previous page](../{tiers[0]}/page-0001.md).\n" if tier != tiers[0] else "")
            (out / tier).mkdir(exist_ok=True)
            (out / tier / f"page-{n:04d}.md").write_text(
                f'---\ntitle: "Page {n}"\nhero: "{image}"\n---\n' + "\n".join(body), encoding="utf-8")
    total = sum(p.stat().st_size for p in (out / "images").iterdir())
    print(f"{n} pages, {total / 1e6:.0f} MB of images, {args.diagrams - diagrams_left} diagrams")


if __name__ == "__main__":
    main()
