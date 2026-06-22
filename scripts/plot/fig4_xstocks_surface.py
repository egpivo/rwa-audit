#!/usr/bin/env python3
"""Fig. 4 — xStocks surface snapshot (reads frozen exchange audit panel)."""

import csv
import json
import os
from pathlib import Path

import matplotlib.pyplot as plt

ROOT = Path(__file__).resolve().parent.parent.parent
BUNDLE_ID = os.environ.get("RWA_AUDIT_BUNDLE", "article3-xstocks-2026-06-12")
BUNDLE_ROOT = ROOT / "artifacts" / "audits" / BUNDLE_ID
LEGACY_DATA = ROOT / "artifacts" / "data"

if (BUNDLE_ROOT / "data" / "depth_vs_volume_panel_publish.csv").exists():
    DATA = BUNDLE_ROOT / "data"
    OUT = BUNDLE_ROOT / "figures" / "xstocks_surface_snapshot.png"
else:
    DATA = LEGACY_DATA
    OUT = ROOT / "artifacts" / "figures" / "xstocks_surface_snapshot.png"

PANEL = DATA / "depth_vs_volume_panel_publish.csv"
MANIFEST = BUNDLE_ROOT / "manifest.json"
if not MANIFEST.exists():
    MANIFEST = DATA / "manifest.json"

BG = "#F7F6F3"
TEXT = "#2B2B2B"
TEXT_SEC = "#5F6368"
GRID = "#D9D7D1"

FIG4_ORDER = [
    ("xStocks platform", "monthly_transfer_volume", "RWA.xyz platform · monthly transfer", "#8A6799"),
    ("xStocks platform", "bridged_token_value_total", "RWA.xyz xStocks · bridged token value", "#4C6A91"),
    ("SPYx", "volume_24h_total", "SPYx · Solana public DEX · 24h pool volume", "#B7905E"),
    ("TSLAx", "volume_24h_total", "TSLAx · Solana public DEX · 24h pool volume", "#6A8F73"),
    ("AAPLx", "pool_tvl_total", "AAPLx · Solana public DEX · pool TVL", "#7C8A78"),
    ("AAPLx", "volume_24h_total", "AAPLx · Solana public DEX · 24h pool volume", "#4C6A91"),
]


def fmt_usd(v: float) -> str:
    if v >= 1e9:
        return f"${v / 1e9:.2f}B"
    if v >= 1e6:
        return f"${v / 1e6:.1f}M"
    if v >= 1e3:
        return f"${v / 1e3:.0f}k"
    return f"${v:,.0f}"


def load_rows():
    with open(PANEL, newline="", encoding="utf-8") as f:
        return list(csv.DictReader(f))


def pick_fig4_rows(rows):
    out = []
    for asset, metric, label, color in FIG4_ORDER:
        if asset == "xStocks platform" and metric == "monthly_transfer_volume":
            matches = [r for r in rows if r["asset_or_example"] == asset and r["metric_type"] == metric]
            r = max(matches, key=lambda x: float(x["metric_value"])) if matches else None
        else:
            r = next(
                (r for r in rows if r["asset_or_example"] == asset and r["metric_type"] == metric),
                None,
            )
        if not r:
            raise SystemExit(f"Missing panel row: {asset} / {metric}")
        out.append((label, float(r["metric_value"]), r["date"], color))
    return out


def main():
    rows = pick_fig4_rows(load_rows())
    plt.rcParams.update({
        "font.family": "sans-serif",
        "font.sans-serif": ["Inter", "Source Sans 3", "Arial", "Helvetica", "DejaVu Sans"],
        "figure.facecolor": BG,
        "axes.facecolor": BG,
        "text.color": TEXT,
    })
    fig, ax = plt.subplots(figsize=(16, 9))
    names = [r[0] for r in rows]
    vals = [r[1] for r in rows]
    dates = [r[2] for r in rows]
    colors = [r[3] for r in rows]
    y_pos = list(range(len(rows)))
    ax.barh(y_pos, vals, color=colors, height=0.58, edgecolor="none")
    ax.invert_yaxis()
    ax.set_xscale("log")
    ax.set_xlabel("USD (log scale)", fontsize=15)
    ax.set_title(
        "Platform metrics and pool metrics sit on different scales",
        fontsize=26,
        fontweight="bold",
        loc="left",
        pad=20,
    )
    ax.set_yticks(y_pos)
    ax.set_yticklabels(names, fontsize=13)
    ax.grid(True, axis="x", color=GRID, linewidth=0.8, alpha=0.9)
    ax.set_axisbelow(True)
    for spine in ("top", "right", "left"):
        ax.spines[spine].set_visible(False)
    ax.spines["bottom"].set_color(GRID)
    xmin, xmax = ax.get_xlim()
    for i, (v, dt) in enumerate(zip(vals, dates)):
        ax.text(v * 1.15, i, f"{fmt_usd(v)} · {dt}", va="center", fontsize=13, color=TEXT)
    ax.set_xlim(xmin, xmax * 2.2)
    fig.subplots_adjust(left=0.34, right=0.96, top=0.92, bottom=0.10)
    OUT.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(OUT, dpi=150, bbox_inches="tight", facecolor=BG)
    plt.close(fig)
    print(f"Wrote {OUT}")
    if MANIFEST.exists():
        print(f"Manifest: {MANIFEST}")


if __name__ == "__main__":
    main()
