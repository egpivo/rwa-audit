#!/usr/bin/env python3
"""Fig. 1 — PAXG public-pool routing dispersion vs COMEX gold stress."""

import csv
from collections import defaultdict
from datetime import datetime
from pathlib import Path

import matplotlib.dates as mdates
import matplotlib.pyplot as plt

ROOT = Path(__file__).resolve().parent.parent.parent
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
from rwa_plot_style import BG, PRODUCT_COLORS, SUBTEXT, TEXT, apply_font, style_axes  # noqa: E402

DATA = ROOT / "data" / "flow" / "panel_daily.csv"
OUT = ROOT / "figures" / "fig1_paxg_reference_timeseries.png"


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
            if r["symbol"] != "PAXG":
                continue
            rows.append(r)
    rows.sort(key=lambda r: r["date"])

    dates = [datetime.strptime(r["date"], "%Y-%m-%d").date() for r in rows]
    gold_z = [float(r["gold_abs_return_robust_z"]) if r.get("gold_abs_return_robust_z") else 0.0 for r in rows]
    disp_z = [float(r["routing_dispersion_robust_z"]) for r in rows]

    fig, (ax1, ax2) = plt.subplots(2, 1, figsize=(12.5, 7.5), sharex=True, facecolor=BG)
    style_axes(ax1, ygrid=True)
    style_axes(ax2, ygrid=True)

    c_gold = "#B08B57"
    c_disp = PRODUCT_COLORS.get("PAXG", "#4C6A91")

    ax1.plot(dates, rolling_mean(gold_z, 3), color=c_gold, linewidth=2.4, label="Gold abs-return z (3D MA)")
    ax2.plot(dates, rolling_mean(disp_z, 7), color=c_disp, linewidth=2.4, label="Routing dispersion z (7D MA)")

    ax1.set_title("Gold stress and PAXG routing dispersion", loc="left", fontsize=22, color=TEXT, fontweight="bold", pad=12)
    ax1.set_ylabel("COMEX GC=F abs-return robust z", fontsize=13, color=TEXT)
    ax2.set_ylabel("Routing dispersion robust z", fontsize=13, color=TEXT)
    ax2.set_xlabel("Date (UTC)", fontsize=13, color=TEXT)
    ax2.xaxis.set_major_formatter(mdates.DateFormatter("%m-%d"))
    ax1.legend(frameon=False, fontsize=11, loc="upper right")
    ax2.legend(frameon=False, fontsize=11, loc="upper right")

    fig.text(
        0.01,
        0.01,
        "Routing dispersion = 1 − top-pool volume share on GeckoTerminal Ethereum public pools. "
        "Descriptive co-movement only; not causal.",
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
