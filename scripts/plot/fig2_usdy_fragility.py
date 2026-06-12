#!/usr/bin/env python3
"""Fig. 2 — USDY thin public-pool activity vs concentration."""

import csv
from datetime import datetime
from pathlib import Path

import matplotlib.dates as mdates
import matplotlib.pyplot as plt

ROOT = Path(__file__).resolve().parent.parent.parent
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
from rwa_plot_style import BG, PRODUCT_COLORS, SUBTEXT, TEXT, apply_font, style_axes  # noqa: E402

DATA = ROOT / "data" / "flow" / "panel_daily.csv"
OUT = ROOT / "figures" / "fig2_usdy_thin_surface_fragility.png"


def rolling_mean(values, window=7):
    out = []
    for i in range(len(values)):
        start = max(0, i - window + 1)
        chunk = values[start : i + 1]
        out.append(sum(chunk) / len(chunk))
    return out


def main():
    apply_font()
    rows = []
    with open(DATA, newline="", encoding="utf-8") as f:
        for r in csv.DictReader(f):
            if r["symbol"] != "USDY":
                continue
            rows.append(r)
    rows.sort(key=lambda r: r["date"])

    dates = [datetime.strptime(r["date"], "%Y-%m-%d").date() for r in rows]
    vol_z = [float(r["volume_robust_z"]) for r in rows]
    share_z = [float(r["top_pool_share_robust_z"]) for r in rows]

    fig, (ax1, ax2) = plt.subplots(2, 1, figsize=(12.5, 7.5), sharex=True, facecolor=BG)
    style_axes(ax1, ygrid=True)
    style_axes(ax2, ygrid=True)

    c_vol = PRODUCT_COLORS.get("USDY", "#6E8FB3")
    c_share = "#8A6799"

    ax1.plot(dates, rolling_mean(vol_z, 3), color=c_vol, linewidth=2.4, label="Pool volume z (3D MA)")
    ax2.plot(dates, rolling_mean(share_z, 7), color=c_share, linewidth=2.4, label="Top-pool share z (7D MA)")

    ax1.set_title("Pool activity and concentration on a thin surface", loc="left", fontsize=22, color=TEXT, fontweight="bold", pad=12)
    ax1.set_ylabel("Daily pool volume robust z", fontsize=13, color=TEXT)
    ax2.set_ylabel("Top-pool concentration robust z", fontsize=13, color=TEXT)
    ax2.set_xlabel("Date (UTC)", fontsize=13, color=TEXT)
    ax2.xaxis.set_major_formatter(mdates.DateFormatter("%m-%d"))
    ax1.legend(frameon=False, fontsize=11, loc="upper right")
    ax2.legend(frameon=False, fontsize=11, loc="upper right")

    fig.text(
        0.01,
        0.01,
        "GeckoTerminal Ethereum public pools only. Descriptive co-movement only; not causal.",
        fontsize=10,
        color=SUBTEXT,
        ha="left",
    )
    fig.tight_layout(rect=[0, 0.04, 1, 1])
    OUT.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(OUT, dpi=160, facecolor=BG, bbox_inches="tight")
    plt.close(fig)
    print(f"Wrote {OUT}")


if __name__ == "__main__":
    main()
