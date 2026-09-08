#!/usr/bin/env python3
"""Check a brand font file for format correctness and UI rendering fit.

Usage:
    python3 tools/check_font.py FONT_FILE [FONT_FILE ...]

Validates local font file(s) against the constraints the UI relies on (see
BRANDING.md, Fonts). Only the file on disk is inspected; the server is not
contacted.

Checks performed (each reported as PASS / WARN / FAIL):
  1. extension - the extension is one of the supported formats
     (.ttf/.otf/.woff/.woff2). A case mismatch is flagged because the
     backend derives the CSS `format()` hint with a case-sensitive match
     (server/src/config.rs), so e.g. `.TTF` yields no format hint.
  2. magic - the file's magic bytes identify the same format the extension
     claims. A renamed/mislabelled file (bytes that disagree with the name)
     would be rejected by the browser.
  3. parse - the font parses as a valid OpenType/Sfnt structure.
  4. variable - whether the font is variable (has `fvar` + `gvar`). The CSS
     injects `font-weight: 100 900`, so a static font only renders its single
     nominal weight and the rest are synthesized.
  5. weight - for variable fonts, whether the `wght` axis covers the weights
     the UI uses (400 normal .. 700 bold).
  6. coverage - whether required characters (basic printable Latin plus the
     typographic punctuation the UI emits) have glyphs.

Checks 3-6 need fontTools. If it is missing they are skipped (as WARN) with a
hint. Install the dependency from apt, not pip:
    sudo apt install python3-fonttools python3-brotli
`python3-brotli` is required to read `.woff2`.

Exit codes: 0 = every check passed, 1 = issues found (any FAIL or WARN),
2 = usage or IO error.
"""

import argparse
import sys
from pathlib import Path

PASS = "PASS"
WARN = "WARN"
FAIL = "FAIL"

# Magic signature for the first 4 bytes of each container, keyed by the
# extension name. woff/woff2 are distinguished by their own header; ttf is the
# TrueType sfnt flavour and otf the CFF/OpenType sfnt flavour.
MAGIC = {
    "woff2": b"wOF2",
    "woff": b"wOFF",
    "ttf": b"\x00\x01\x00\x00",
    "otf": b"OTTO",
}
SUPPORTED = tuple(MAGIC)

# Font weights the UI renders: 400 normal, 500 font-medium, 600
# font-semibold, 700 font-bold.
UI_WEIGHTS = (400, 500, 600, 700)

# Basic printable Latin that must render for the UI to be usable at all.
CORE = {c for c in range(0x20, 0x7F)}
# Typographic punctuation the UI is likely to emit.
EXTRA = {
    "\u00a0",  # nbsp
    "\u00a9",  # (c)
    "\u00ae",  # (r)
    "\u2013",  # en dash
    "\u2014",  # em dash
    "\u2018",  # left single quote
    "\u2019",  # right single quote
    "\u201c",  # left double quote
    "\u201d",  # right double quote
    "\u2026",  # ellipsis
    "\u2212",  # minus sign
}


def die(message):
    print(f"error: {message}", file=sys.stderr)
    raise SystemExit(2)


def detect_format(head):
    for name, magic in MAGIC.items():
        if head.startswith(magic):
            return name
    return "unknown"


def check_extension(path, results):
    suffix = path.suffix            # e.g. ".woff2"
    body = suffix.lstrip(".")       # "woff2"
    declared = body.lower()
    if declared not in SUPPORTED:
        shown = suffix or "(none)"
        results.append(
            (FAIL, "extension", f"'{shown}' not supported "
             f"(need .{'/'.join(SUPPORTED)})")
        )
        return declared
    if body != declared:
        results.append(
            (WARN, "extension", f"uppercase '{suffix}': backend sets no "
             f"format() hint (case-sensitive match)")
        )
    else:
        results.append((PASS, "extension", declared))
    return declared


def check_magic(path, declared, results):
    try:
        with path.open("rb") as fh:
            head = fh.read(4)
    except OSError as exc:
        die(f"cannot read {path}: {exc}")
    detected = detect_format(head)
    if detected == "unknown":
        results.append(
            (FAIL, "magic", f"unrecognized magic {head.hex()} - not a font")
        )
        return detected
    if declared in MAGIC:
        if detected == declared:
            results.append((PASS, "magic", f"bytes match '{detected}'"))
        else:
            results.append(
                (FAIL, "magic", f"extension says '{declared}' but bytes are "
                 f"'{detected}' (head={head.hex()})")
            )
    else:
        results.append(
            (WARN, "magic", f"detected '{detected}' but extension not "
             f"supported")
        )
    return detected


def check_deep(path, detected, results):
    """Run the fontTools-based checks (parse/variable/weight/coverage)."""
    try:
        from fontTools.ttLib import TTFont
    except ImportError:
        hint = "fontTools missing (sudo apt install python3-fonttools " \
               "python3-brotli)"
        for name in ("parse", "variable", "weight", "coverage"):
            results.append((WARN, name, f"skipped: {hint}"))
        return

    try:
        font = TTFont(path)
    except Exception as exc:  # noqa: BLE001 - report any parse failure
        if detected == "woff2":
            hint = " (need python3-brotli for .woff2)"
        else:
            hint = ""
        results.append((FAIL, "parse", f"fontTools could not parse: {exc}{hint}"))
        for name in ("variable", "weight", "coverage"):
            results.append((WARN, name, "skipped: parse failed"))
        return

    results.append(
        (PASS, "parse", f"valid OpenType/Sfnt ({len(font.keys())} tables)")
    )

    # 4. variable
    is_variable = "fvar" in font and "gvar" in font
    if is_variable:
        results.append((PASS, "variable", "variable font (fvar + gvar)"))
    else:
        results.append(
            (WARN, "variable",
             "static font: CSS font-weight:100 900 will not produce real "
             "weights (rest are synthesized)")
        )

    # 5. weight
    if is_variable:
        axes = {a.axisTag: a for a in font["fvar"].axes}
        wght = axes.get("wght")
        if wght is None:
            results.append((WARN, "weight", "variable font has no wght axis"))
        else:
            lo, hi = int(wght.minValue), int(wght.maxValue)
            uncovered = [w for w in UI_WEIGHTS if not lo <= w <= hi]
            if not uncovered:
                results.append(
                    (PASS, "weight", f"wght axis {lo}-{hi} covers UI weights "
                     f"{UI_WEIGHTS[0]}-{UI_WEIGHTS[-1]}")
                )
            else:
                results.append(
                    (WARN, "weight", f"wght axis {lo}-{hi} does not cover "
                     f"{uncovered} (they will clamp to the nearest weight)")
                )
    else:
        nominal = None
        if "OS/2" in font:
            nominal = getattr(font["OS/2"], "usWeightClass", None)
        if nominal is not None:
            results.append(
                (WARN, "weight", f"static font nominal weight {nominal}; "
                 f"CSS range 100-900 has no effect")
            )
        else:
            results.append((WARN, "weight", "static font; nominal weight "
                                 "unknown"))

    # 6. coverage
    cmap = font.getBestCmap() or {}
    missing_core = sorted(c for c in CORE if c not in cmap)
    missing_extra = sorted(ord(ch) for ch in EXTRA if ord(ch) not in cmap)
    if missing_core:
        sample = ", ".join(repr(chr(c)) for c in missing_core[:16])
        more = f" (+{len(missing_core) - 16} more)" if len(missing_core) > 16 else ""
        results.append(
            (FAIL, "coverage", f"{len(missing_core)} basic-Latin glyphs "
             f"missing: {sample}{more}")
        )
    elif missing_extra:
        sample = ", ".join(repr(chr(c)) for c in missing_extra)
        results.append((WARN, "coverage", f"missing punctuation: {sample}"))
    else:
        results.append((PASS, "coverage", "all required glyphs present"))


def check_file(path):
    if not path.is_file():
        die(f"not a file: {path}")
    size = path.stat().st_size
    if size == 0:
        print(f"== {path} ==")
        print(f"  {FAIL}  file  empty (0 bytes)")
        return FAIL

    results = []
    declared = check_extension(path, results)
    detected = check_magic(path, declared, results)
    check_deep(path, detected, results)

    print(f"== {path} ({size} bytes, detected: {detected}) ==")
    width = max(len(name) for _, name, _ in results)
    for status, name, detail in results:
        print(f"  {status:<4} {name.ljust(width)}  {detail}")
    return worst_status(results)


def worst_status(results):
    statuses = [s for s, _, _ in results]
    if FAIL in statuses:
        return FAIL
    if WARN in statuses:
        return WARN
    return PASS


def main(argv=None):
    parser = argparse.ArgumentParser(
        description="Check a brand font file for format correctness and "
                    "UI rendering fit."
    )
    parser.add_argument(
        "fonts", nargs="+", type=Path,
        help="font file(s) to check (.ttf/.otf/.woff/.woff2)",
    )
    args = parser.parse_args(argv)

    has_issue = False
    for path in args.fonts:
        if check_file(path) != PASS:
            has_issue = True
    return 1 if has_issue else 0


if __name__ == "__main__":
    sys.exit(main())
