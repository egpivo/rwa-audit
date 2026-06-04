"""Shared Zen-blog matplotlib style for RWA article figures."""

from matplotlib import font_manager
import matplotlib.pyplot as plt

BG = "#F7F6F3"
TEXT = "#2B2B2B"
SUBTEXT = "#5F6368"
GRID = "#D9D7D1"

# Workflow-type colors
PERMISSIONED = "#8A6799"  # muted plum (Ethereum family)
HYBRID_NOTE = "#6E8FB3"   # blue-gray
COMMODITY_A = "#4C6A91"   # muted steel blue (PAXG)
COMMODITY_B = "#7A6E8A"   # muted slate (XAUT — distinct from PAXG at small size)
RECORDKEEPING = "#B08B57"
PLATFORM = "#4E8D64"

PRODUCT_COLORS = {
    "BUIDL": PERMISSIONED,
    "USDY": HYBRID_NOTE,
    "BENJI": RECORDKEEPING,
    "PAXG": COMMODITY_A,
    "XAUT": COMMODITY_B,
    "xStocks platform": PLATFORM,
}

WORKFLOW_COLORS = {
    "Permissioned treasury settlement rail": PERMISSIONED,
    "Tokenized-note subscription/routing surface": HYBRID_NOTE,
    "Transfer-agent recordkeeping extension": RECORDKEEPING,
    "Open commodity transfer surface": COMMODITY_A,
    "Tokenized stock / on-chain market interface": PLATFORM,
}


def apply_font():
    for name in ["Inter", "Source Sans 3", "Arial", "Helvetica"]:
        if name in [f.name for f in font_manager.fontManager.ttflist]:
            plt.rcParams["font.family"] = name
            break


def style_axes(ax, *, ygrid=True, xgrid=False):
    ax.set_facecolor(BG)
    if ygrid:
        ax.yaxis.grid(True, color=GRID, linewidth=0.7, zorder=0)
    if xgrid:
        ax.xaxis.grid(True, color=GRID, linewidth=0.7, zorder=0)
    ax.set_axisbelow(True)
    ax.spines["top"].set_visible(False)
    ax.spines["right"].set_visible(False)
    ax.spines["left"].set_color(GRID)
    ax.spines["bottom"].set_color(GRID)
    ax.tick_params(axis="both", labelsize=12, colors=TEXT)
