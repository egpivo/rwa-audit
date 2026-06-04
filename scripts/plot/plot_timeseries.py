#!/usr/bin/env python3
"""
Plot 30-day observable activity time-series from data/rwa_activity_daily_30d.csv.

Data collection is handled by the Rust binary `rwa-activity`.
"""

import csv
import datetime as dt
import sys
from collections import defaultdict
from pathlib import Path

import matplotlib.dates as mdates
import matplotlib.pyplot as plt
import matplotlib.ticker as mticker

ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))
from rwa_plot_style import (  # noqa: E402
    BG,
    PRODUCT_COLORS,
    TEXT,
    apply_font,
    style_axes,
)

DATA_CSV = ROOT / "data" / "rwa_activity_daily_30d.csv"
FIG_OUT = ROOT / "figures" / "rwa_activity_timeseries.png"


def rolling_mean(values, window=7):
    out = []
    for i in range(len(values)):
        start = max(0, i - window + 1)
        chunk = values[start : i + 1]
        out.append(sum(chunk) / len(chunk))
    return out


def _load_series(csv_path: Path):
    by_sym = defaultdict(lambda: {"dates": [], "vol": [], "active": []})
    with open(csv_path, "r", encoding="utf-8") as f:
        for r in csv.DictReader(f):
            if r.get("include_in_figure") != "yes":
                continue
            sym = r["product_or_platform"]
            by_sym[sym]["dates"].append(dt.datetime.strptime(r["date"], "%Y-%m-%d").date())
            by_sym[sym]["vol"].append(float(r["volume_usd"]) if r["volume_usd"] else 0.0)
            by_sym[sym]["active"].append(int(r["active_user_count"]))

    series = {}
    for sym, d in by_sym.items():
        order = sorted(range(len(d["dates"])), key=lambda i: d["dates"][i])
        dates = [d["dates"][i] for i in order]
        vol = [d["vol"][i] for i in order]
        active = [d["active"][i] for i in order]
        series[sym] = {
            "dates": dates,
            "volume_ma7": rolling_mean(vol, 7),
            "active_ma7": rolling_mean([float(a) for a in active], 7),
        }
    return series


def _usd_formatter():
    return mticker.FuncFormatter(
        lambda x, _: f"${x/1e6:.0f}M" if x >= 1e6 else (f"${x/1e3:.0f}K" if x >= 1e3 else f"${x:.0f}")
    )


def _plot_group(ax, series, symbols, *, metric, log_volume=False, linestyles=None):
    linestyles = linestyles or {}
    for sym in symbols:
        if sym not in series:
            continue
        s = series[sym]
        c = PRODUCT_COLORS.get(sym, "#777777")
        ls = linestyles.get(sym, "-")
        if metric == "volume":
            y = [max(v, 1.0) for v in s["volume_ma7"]]
        else:
            y = s["active_ma7"]
        ax.plot(s["dates"], y, label=sym, color=c, linewidth=2.2, linestyle=ls, solid_capstyle="round")
    if log_volume:
        ax.set_yscale("log")
        ax.yaxis.set_major_formatter(_usd_formatter())
    style_axes(ax, ygrid=True)
    ax.xaxis.set_major_formatter(mdates.DateFormatter("%m-%d"))
    ax.xaxis.set_major_locator(mdates.DayLocator(interval=6))
    ax.legend(frameon=False, fontsize=11, loc="upper right")


def plot_timeseries_from_csv(csv_path: Path = DATA_CSV, out_path: Path = FIG_OUT):
    """Four-panel figure: split permissioned vs commodity so scales stay readable."""
    apply_font()
    series = _load_series(csv_path)

    fig, axes = plt.subplots(2, 2, figsize=(13.5, 8.0), facecolor=BG, sharex=True)
    ax_a, ax_b = axes[0]
    ax_c, ax_d = axes[1]

    perm = ["BUIDL", "USDY"]
    comm = ["PAXG", "XAUT"]
    comm_ls = {"PAXG": "-", "XAUT": (0, (4, 2))}

    _plot_group(ax_a, series, perm, metric="volume", log_volume=True)
    _plot_group(ax_b, series, comm, metric="volume", log_volume=True, linestyles=comm_ls)
    _plot_group(ax_c, series, perm, metric="active")
    _plot_group(ax_d, series, comm, metric="active", linestyles=comm_ls)

    ax_a.set_title("Panel A — Permissioned volume", loc="left", fontsize=13, color=TEXT, pad=8)
    ax_b.set_title("Panel B — Commodity volume", loc="left", fontsize=13, color=TEXT, pad=8)
    ax_c.set_title("Panel C — Permissioned active senders", loc="left", fontsize=13, color=TEXT, pad=8)
    ax_d.set_title("Panel D — Commodity active senders", loc="left", fontsize=13, color=TEXT, pad=8)

    ax_a.set_ylabel("Transfer volume, 7D MA (USD)", fontsize=12, color=TEXT)
    ax_b.set_ylabel("Transfer volume, 7D MA (USD)", fontsize=12, color=TEXT)
    ax_c.set_ylabel("Active senders, 7D MA", fontsize=12, color=TEXT)
    ax_d.set_ylabel("Active senders, 7D MA", fontsize=12, color=TEXT)
    ax_c.set_xlabel("Date (UTC)", fontsize=12, color=TEXT)
    ax_d.set_xlabel("Date (UTC)", fontsize=12, color=TEXT)

    fig.subplots_adjust(left=0.07, right=0.98, top=0.96, bottom=0.08, hspace=0.32, wspace=0.22)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(out_path, dpi=160, facecolor=BG, bbox_inches="tight")
    plt.close(fig)
    print(f"Wrote {out_path}")


if __name__ == "__main__":
    plot_timeseries_from_csv()
