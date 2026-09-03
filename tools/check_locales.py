#!/usr/bin/env python3
"""Check locale files for consistency and usage.

Usage:
    python3 tools/check_locales.py LOCALES_DIR SOURCE_DIR [--reference en]

Checks performed:
  1. consistency - every locale in LOCALES_DIR (nested JSON objects are
     flattened to dot-separated keys) is compared against the reference
     locale: keys missing from the locale and extra keys are reported.
  2. unused - keys defined in the reference locale that are never
     referenced in the scanned source files.
  3. missing - dotted t('...') / $t('...') literals found in the source
     files that are absent from every locale file.

Only text files are scanned (valid UTF-8, no NUL bytes). The directories
.git, coverage, dist, node_modules, target and LOCALES_DIR itself are
skipped. Dynamically constructed keys cannot be detected.

Exit codes: 0 = clean, 1 = issues found, 2 = usage or IO error.
"""

import argparse
import json
import re
import sys
from pathlib import Path

SKIP_DIRS = {".git", "coverage", "dist", "node_modules", "target"}

# t('a.b') / $t("a.b") / global.t('a.b'); the lookbehind avoids matches
# inside longer identifiers such as submit(...) or format(...).
USAGE_RE = re.compile(r"(?<![\w$])\$?t\(\s*['\"]([A-Za-z0-9_][A-Za-z0-9_.-]*)['\"]")


def die(message):
    print(f"error: {message}", file=sys.stderr)
    raise SystemExit(2)


def flatten(node, prefix, out):
    for key, value in node.items():
        path = f"{prefix}.{key}" if prefix else str(key)
        if isinstance(value, dict):
            flatten(value, path, out)
        else:
            out.add(path)


def load_locales(locales_dir):
    locales = {}
    for path in sorted(locales_dir.glob("*.json")):
        try:
            data = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, ValueError) as exc:
            die(f"cannot read {path}: {exc}")
        if not isinstance(data, dict):
            die(f"{path}: top-level JSON value must be an object")
        keys = set()
        flatten(data, "", keys)
        locales[path.stem] = keys
    return locales


def iter_text_files(source_dir, locales_dir):
    locales_resolved = locales_dir.resolve()
    for path in sorted(source_dir.rglob("*")):
        if not path.is_file():
            continue
        if any(part in SKIP_DIRS for part in path.parts):
            continue
        try:
            if locales_resolved in path.resolve().parents:
                continue
        except OSError:
            pass
        try:
            data = path.read_bytes()
        except OSError:
            continue
        if b"\x00" in data:
            continue
        try:
            yield path, data.decode("utf-8")
        except UnicodeDecodeError:
            continue


def main(argv=None):
    parser = argparse.ArgumentParser(
        description="Check locale files for consistency and usage."
    )
    parser.add_argument("locales_dir", type=Path)
    parser.add_argument("source_dir", type=Path)
    parser.add_argument(
        "--reference", default="en", help="reference locale code (default: en)"
    )
    args = parser.parse_args(argv)

    if not args.locales_dir.is_dir():
        die(f"locales directory not found: {args.locales_dir}")
    if not args.source_dir.is_dir():
        die(f"source directory not found: {args.source_dir}")

    locales = load_locales(args.locales_dir)
    if not locales:
        die(f"no *.json files found in {args.locales_dir}")
    if args.reference not in locales:
        die(
            f"reference locale '{args.reference}.json' not found in "
            f"{args.locales_dir} (available: {', '.join(sorted(locales))})"
        )
    reference = args.reference
    ref_keys = locales[reference]
    all_keys = set().union(*locales.values())

    print(f"== consistency (reference: {reference}) ==")
    consistency_clean = True
    for code in sorted(locales):
        if code == reference:
            continue
        keys = locales[code]
        missing = sorted(ref_keys - keys)
        extra = sorted(keys - ref_keys)
        if not missing and not extra:
            print(f"  {code}: OK")
            continue
        consistency_clean = False
        if missing:
            print(f"  {code}: missing from locale: {', '.join(missing)}")
        if extra:
            print(f"  {code}: extra in locale: {', '.join(extra)}")

    texts = list(iter_text_files(args.source_dir, args.locales_dir))

    print(f"== unused keys (defined in {reference}, not referenced in {args.source_dir}) ==")
    unused = []
    for key in sorted(ref_keys):
        pattern = re.compile(re.escape(key) + r"(?![.\w])")
        if not any(pattern.search(text) for _, text in texts):
            unused.append(key)
    if unused:
        for key in unused:
            print(f"  {key}")
    else:
        print("  OK")

    print("== missing keys (referenced in source, not in any locale) ==")
    used = {}
    for path, text in texts:
        for match in USAGE_RE.finditer(text):
            key = match.group(1)
            if "." not in key or key in used:
                continue
            line = text.count("\n", 0, match.start()) + 1
            used[key] = f"{path}:{line}"
    missing_used = sorted(key for key in used if key not in all_keys)
    if missing_used:
        for key in missing_used:
            print(f"  {key} (first use: {used[key]})")
    else:
        print("  OK")

    issues = (not consistency_clean) + len(unused) + len(missing_used)
    return 1 if issues else 0


if __name__ == "__main__":
    sys.exit(main())
