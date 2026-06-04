"""
RWA article figures — Zen blog style
  figures/rwa_stack.png              — verification stack (Fig. 1)
  figures/rwa_activity_surface.png   — volume vs active user scatter (Fig. 2)
  figures/rwa_activity_intensity.png — intensity by visible participant (Fig. 3)
"""

import csv
import os
import sys
from pathlib import Path

import matplotlib.pyplot as plt
import matplotlib.ticker as ticker
import numpy as np

ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))
from rwa_plot_style import (  # noqa: E402
    BG,
    PRODUCT_COLORS,
    SUBTEXT,
    TEXT,
    WORKFLOW_COLORS,
    apply_font,
    style_axes,
)

import matplotlib.patches as mpatches

OUT = ROOT / "figures"
DATASET = ROOT / "article" / "writing" / "rwa_activity_surface_dataset.csv"
os.makedirs(OUT, exist_ok=True)
apply_font()


def _load_main_chart_rows():
    rows = []
    with open(DATASET, "r", encoding="utf-8") as f:
        for r in csv.DictReader(f):
            if r.get("comparable_in_main_chart") != "yes":
                continue
            if not r.get("volume_usd") or not r.get("active_user_count"):
                continue
            vol = float(r["volume_usd"])
            active = float(r["active_user_count"])
            if active <= 0:
                continue
            dist = float(r["distributed_value_usd"]) if r.get("distributed_value_usd") else None
            rows.append(
                {
                    "name": r["product_or_platform"],
                    "workflow": r["workflow_type"],
                    "volume": vol,
                    "active": active,
                    "distributed": dist,
                }
            )
    return rows


def make_activity_surface_scatter():
    rows = _load_main_chart_rows()
    fig, ax = plt.subplots(figsize=(10, 7), facecolor=BG)
    style_axes(ax, ygrid=True, xgrid=True)

    for r in rows:
        name = r["name"]
        c = PRODUCT_COLORS.get(name, WORKFLOW_COLORS.get(r["workflow"], "#777777"))
        size = 120
        if r["distributed"]:
            size = 80 + 420 * (r["distributed"] / max(x["distributed"] or 0 for x in rows))
        ax.scatter(
            r["active"],
            r["volume"],
            s=size,
            color=c,
            alpha=0.85,
            edgecolors="#FFFFFF",
            linewidths=0.8,
            zorder=3,
        )
        ax.annotate(
            name.replace(" platform", ""),
            (r["active"], r["volume"]),
            textcoords="offset points",
            xytext=(6, 4),
            fontsize=11,
            color=TEXT,
        )

    ax.set_xscale("log")
    ax.set_yscale("log")
    ax.set_xlabel("Active user base — monthly unique senders / addresses (log scale)", fontsize=14, color=TEXT)
    ax.set_ylabel("Observable volume — May 2026 transfer volume USD (log scale)", fontsize=14, color=TEXT)
    ax.set_title("RWA activity surface", fontsize=22, color=TEXT, fontweight="bold", loc="left", pad=12)
    ax.xaxis.set_major_formatter(
        ticker.FuncFormatter(lambda x, _: f"{int(x)}" if x < 1000 else f"{x/1000:.0f}K")
    )
    ax.yaxis.set_major_formatter(
        ticker.FuncFormatter(
            lambda x, _: f"${x/1e9:.1f}B" if x >= 1e9 else (f"${x/1e6:.0f}M" if x >= 1e6 else f"${x/1e3:.0f}K")
        )
    )

    legend_handles = [
        mpatches.Patch(color=PRODUCT_COLORS["BUIDL"], label="Permissioned rail"),
        mpatches.Patch(color=PRODUCT_COLORS["USDY"], label="Tokenized note"),
        mpatches.Patch(color=PRODUCT_COLORS["PAXG"], label="Open commodity"),
        mpatches.Patch(color=PRODUCT_COLORS["xStocks platform"], label="Platform reference"),
    ]
    ax.legend(handles=legend_handles, frameon=False, fontsize=11, loc="lower right")

    out = OUT / "rwa_activity_surface.png"
    fig.savefig(str(out), dpi=160, facecolor=BG, bbox_inches="tight")
    plt.close(fig)
    print(f"Saved {out}")


def make_rwa_stack():
    layers = [
        ("Real-world asset", "Treasury note · Gold bar · Loan · ETF share", "#DEDAD4", TEXT, ""),
        ("Legal / economic claim", "Prospectus · Offering memorandum · Terms of service", "#DEDAD4", TEXT, ""),
        ("Issuer / SPV / Fund / Custodian", "NAV calculation · Custody · Redemption · Eligibility", "#DEDAD4", TEXT, ""),
        ("Token contract", "ABI · Admin controls · Supply · Standard", "#C7D4C5", TEXT, "On-chain (if source verified)"),
        ("Permissioning · Compliance · Mint-burn logic", "Whitelist enforcement · Gating · Supply events", "#C7D4C5", TEXT, "Partially observable"),
        ("Blockchain ledger", "Transfer events · Balances · Supply changes · Timestamps", "#A8C4A8", TEXT, "Directly observable"),
        ("Settlement · DeFi · Institutional workflow", "Stablecoin rails · Collateral protocols · Clearing", "#E8D5B0", TEXT, "Syntactically visible\nSemantically opaque"),
    ]

    n = len(layers)
    fig_h = 2.0 + n * 0.85
    fig, ax = plt.subplots(figsize=(13, fig_h), facecolor=BG)
    ax.set_facecolor(BG)
    ax.set_xlim(0, 10)
    ax.set_ylim(0, n)
    ax.axis("off")

    box_x, box_w, box_h, gap = 0.25, 7.5, 0.72, 0.13
    for i, (label, sub, fill, tc, obs) in enumerate(reversed(layers)):
        y = i * (box_h + gap)
        rect = mpatches.FancyBboxPatch(
            (box_x, y), box_w, box_h, boxstyle="round,pad=0.05",
            linewidth=0.8, edgecolor="#BFBAB4", facecolor=fill,
        )
        ax.add_patch(rect)
        ax.text(box_x + 0.22, y + box_h * 0.63, label, fontsize=13.5, fontweight="bold", color=tc, va="center")
        ax.text(box_x + 0.22, y + box_h * 0.28, sub, fontsize=10.5, color=SUBTEXT, va="center")
        if obs:
            ax.text(box_x + box_w - 0.18, y + box_h * 0.5, obs, fontsize=10, color="#4A4A4A", va="center", ha="right", style="italic")
        if i < n - 1:
            ax_center = box_x + box_w / 2
            ax.annotate("", xy=(ax_center, y + box_h + gap * 0.05), xytext=(ax_center, y + box_h + gap * 0.95),
                        arrowprops=dict(arrowstyle="-|>", color="#9A9590", lw=1.2))

    ax.set_title("RWA verification stack", fontsize=22, color=TEXT, fontweight="bold", pad=10, loc="left")
    out = OUT / "rwa_stack.png"
    fig.savefig(str(out), dpi=160, facecolor=BG, bbox_inches="tight")
    plt.close(fig)
    print(f"Saved {out}")


def make_activity_intensity():
    rows = []
    with open(DATASET, "r", encoding="utf-8") as f:
        for r in csv.DictReader(f):
            if r.get("comparable_in_main_chart") != "yes":
                continue
            if not r.get("volume_usd") or not r.get("active_user_count"):
                continue
            volume = float(r["volume_usd"])
            active = float(r["active_user_count"])
            if active <= 0:
                continue
            rows.append((r["product_or_platform"], volume / active, r["workflow_type"]))

    rows.sort(key=lambda x: x[1], reverse=True)
    labels = [r[0].replace(" platform", "") for r in rows]
    values = [r[1] for r in rows]
    colors = [PRODUCT_COLORS.get(r[0], WORKFLOW_COLORS.get(r[2], "#777777")) for r in rows]

    fig, ax = plt.subplots(figsize=(11, 5.5), facecolor=BG)
    style_axes(ax, xgrid=True)
    y_pos = np.arange(len(labels))
    ax.barh(y_pos, values, color=colors, height=0.55, edgecolor="none")
    ax.invert_yaxis()
    ax.set_yticks(y_pos)
    ax.set_yticklabels(labels, fontsize=12, color=TEXT)
    ax.set_xlabel("Observable volume per active sender/address (USD, log scale)", fontsize=14, color=TEXT)
    ax.set_xscale("log")
    ax.set_xlim(1_000, max(values) * 1.8)
    ax.xaxis.set_major_formatter(
        ticker.FuncFormatter(lambda x, _: f"${x/1_000_000:.1f}M" if x >= 1_000_000 else f"${x/1000:.0f}K")
    )
    ax.tick_params(axis="y", length=0)

    for b, v in zip(ax.patches, values):
        label = f"${v/1_000_000:.2f}M" if v >= 1_000_000 else f"${v/1000:.1f}K"
        ax.text(v * 1.12, b.get_y() + b.get_height() / 2, label, va="center", ha="left", fontsize=11, color=TEXT)

    ax.set_title("Activity intensity by visible participant", fontsize=22, color=TEXT, fontweight="bold", loc="left", pad=12)
    out = OUT / "rwa_activity_intensity.png"
    fig.savefig(str(out), dpi=160, facecolor=BG, bbox_inches="tight")
    plt.close(fig)
    print(f"Saved {out}")


if __name__ == "__main__":
    make_rwa_stack()
    make_activity_surface_scatter()
    make_activity_intensity()
