#!/usr/bin/env -S uv run --quiet
# /// script
# dependencies = ["polars"]
# ///
"""Analyze a samply profile (with --unstable-presymbolicate sidecar) and print bottlenecks."""

import json
import sys
from pathlib import Path

import polars as pl


def load_symbols(syms_path):
    """Build address -> symbol_name lookup from syms.json sidecar."""
    with open(syms_path) as f:
        syms = json.load(f)
    st = syms["string_table"]

    for entry in syms["data"]:
        if "calepin" not in entry.get("debug_name", "").lower():
            continue

        # symbol_table: [{rva, size, symbol, frames}, ...]
        sym_list = entry["symbol_table"]

        # known_addresses: [[address, symbol_table_index], ...]
        addr_to_name = {}
        for addr, idx in entry["known_addresses"]:
            addr_to_name[addr] = st[sym_list[idx]["symbol"]]

        # Also build RVA range lookup as fallback
        rva_ranges = []
        for sym in sym_list:
            rva_ranges.append((sym["rva"], sym["rva"] + sym["size"], st[sym["symbol"]]))

        return addr_to_name, rva_ranges

    return {}, []


def resolve_address(addr, addr_to_name, rva_ranges):
    if addr in addr_to_name:
        return addr_to_name[addr]
    for start, end, name in rva_ranges:
        if start <= addr < end:
            return name
    return None


def simplify_name(name):
    # Strip hash suffix (e.g., ::h1a2b3c4d5e6f7g8)
    if "::" in name:
        parts = name.split("::")
        if len(parts[-1]) == 17 and parts[-1].startswith("h"):
            parts = parts[:-1]
        name = "::".join(parts)
    return name


def categorize(name):
    """Map a function name to a pipeline category."""
    categories = [
        ("comrak::", "markdown (comrak)"),
        ("syntect::", "highlighting (syntect)"),
        ("minijinja::", "templates (minijinja)"),
        ("hayagriva::", "bibliography (hayagriva)"),
        ("toml", "config (toml)"),
        ("regex::", "regex"),
        ("alloc::", "allocator"),
    ]
    lower = name.lower()
    for pattern, cat in categories:
        if pattern in lower:
            return cat

    if "calepin" in lower:
        parts = lower.split("::")
        if len(parts) >= 2:
            return f"calepin::{parts[1]}" if "calepin" in parts[0] else parts[0]
        return "calepin"

    return "other"


def analyze_profile(profile_path):
    syms_path = str(profile_path).replace(".json", ".syms.json")
    if not Path(syms_path).exists():
        print(f"Error: sidecar {syms_path} not found. Use --unstable-presymbolicate.")
        sys.exit(1)

    with open(profile_path) as f:
        profile = json.load(f)

    addr_to_name, rva_ranges = load_symbols(syms_path)

    # Find main thread (most samples)
    threads = profile["threads"]
    main = max(threads, key=lambda t: t["samples"]["length"])

    ft = main["frameTable"]
    func_t = main["funcTable"]
    sa = main["stringArray"]
    st = main["stackTable"]
    samples = main["samples"]
    rt = main["resourceTable"]

    # Identify calepin resource indices
    calepin_libs = {
        i for i, lib in enumerate(profile["libs"]) if "calepin" in lib.get("name", "").lower()
    }
    calepin_resources = {i for i in range(rt["length"]) if rt["lib"][i] in calepin_libs}

    n_samples = samples["length"]
    sample_weights = samples.get("weight")

    # Collect per-sample data
    func_rows = []  # per-function: self_weight and total_weight (deduped by function)
    cat_rows = []  # per-category: total_weight (deduped by category per sample)

    for i in range(n_samples):
        weight = sample_weights[i] if sample_weights else 1
        stack_idx = samples["stack"][i]
        if stack_idx is None:
            continue

        depth = 0
        seen_funcs = set()
        seen_cats = set()
        while stack_idx is not None:
            frame_idx = st["frame"][stack_idx]
            func_idx = ft["func"][frame_idx]
            addr = ft["address"][frame_idx]
            resource = func_t["resource"][func_idx]

            if resource in calepin_resources:
                name = resolve_address(addr, addr_to_name, rva_ranges)
                if name:
                    name = simplify_name(name)
                    cat = categorize(name)
                    is_self = depth == 0

                    if name not in seen_funcs:
                        func_rows.append((name, cat, weight if is_self else 0, weight))
                        seen_funcs.add(name)
                    elif is_self:
                        func_rows.append((name, cat, weight, 0))

                    if cat not in seen_cats:
                        cat_rows.append((cat, weight if is_self else 0, weight))
                        seen_cats.add(cat)
                    elif is_self:
                        cat_rows.append((cat, weight, 0))

            stack_idx = st["prefix"][stack_idx]
            depth += 1

    if not func_rows:
        print("No calepin samples found in profile.")
        sys.exit(1)

    total_weight = sum(sample_weights) if sample_weights else n_samples

    df = pl.DataFrame(
        func_rows, schema=["function", "category", "self_weight", "total_weight"], orient="row"
    )

    by_func = (
        df.group_by("function")
        .agg(pl.col("self_weight").sum(), pl.col("total_weight").sum())
        .with_columns(
            self_pct=(pl.col("self_weight") / total_weight * 100),
            total_pct=(pl.col("total_weight") / total_weight * 100),
        )
    )

    print(f"\n{'=' * 70}")
    print(f"Profile: {n_samples} samples")
    print(f"{'=' * 70}")

    print(f"\n--- Top 20 by self time (where CPU spends time) ---\n")
    top_self = by_func.sort("self_weight", descending=True).head(20)
    for row in top_self.iter_rows(named=True):
        if row["self_pct"] < 0.1:
            break
        print(f"  {row['self_pct']:5.1f}%  {row['function'][:80]}")

    print(f"\n--- Top 20 by total time (on-stack, includes callees) ---\n")
    top_total = by_func.sort("total_weight", descending=True).head(20)
    for row in top_total.iter_rows(named=True):
        if row["total_pct"] < 0.1:
            break
        print(f"  {row['total_pct']:5.1f}%  {row['function'][:80]}")

    # --- By category (deduped per sample) ---
    cat_df = pl.DataFrame(cat_rows, schema=["category", "self_weight", "total_weight"], orient="row")
    by_cat = (
        cat_df.group_by("category")
        .agg(pl.col("self_weight").sum(), pl.col("total_weight").sum())
        .with_columns(
            self_pct=(pl.col("self_weight") / total_weight * 100),
            total_pct=(pl.col("total_weight") / total_weight * 100),
        )
        .sort("self_weight", descending=True)
    )

    print(f"\n--- By category ---\n")
    print(f"  {'self':>6}  {'total':>6}  category")
    print(f"  {'----':>6}  {'-----':>6}  --------")
    for row in by_cat.iter_rows(named=True):
        if row["self_pct"] < 0.1 and row["total_pct"] < 0.5:
            continue
        print(f"  {row['self_pct']:5.1f}%  {row['total_pct']:5.1f}%  {row['category']}")

    print()


if __name__ == "__main__":
    if len(sys.argv) != 2:
        print(f"Usage: {sys.argv[0]} <profile.json>")
        sys.exit(1)
    analyze_profile(sys.argv[1])
