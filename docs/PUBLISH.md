# Publish workflow

Frozen audit bundles are the publish contract. Live collectors write scratch data; freeze promotes evidence into `artifacts/audits/{audit-id}/`.

## Article 1 — Registry + activity

```bash
make collect    # → data/rwa_*.csv
make activity   # → data/rwa_activity_daily_30d.csv
cargo run --bin rwa-freeze -- promote article1-registry-2026-06
```

Bundle output: `artifacts/audits/article1-registry-2026-06/{manifest.json,data/,figures/}`

## Article 2 — Flow

```bash
make flow-panel
make flow-quotes
# optional: cargo run --bin rwa-flow-tx -- 0x<hash>
```

Outputs stay in `data/flow/` (live snapshot, also git-tracked).

## Article 3 — Exchange (xStocks)

```bash
make exchange-freeze
# or one step (freeze + promote):
cargo run --bin rwa-freeze -- exchange

make figures
```

1. `rwa-exchange-freeze` writes flat evidence to `artifacts/data/` and `manifest.json`
2. `rwa-freeze promote article3-xstocks-2026-06-12` copies into `artifacts/audits/.../` with bundle-relative evidence paths
3. `fig4_xstocks_surface.py` reads panel + manifest (set `RWA_AUDIT_BUNDLE=article3-xstocks-2026-06-12` for bundle paths)

### Live refresh (non-publish)

Live exchange runs write to `data/exchange-live/` only. They do not modify `artifacts/data/` or promote bundles.

```bash
# Live API probe (Gecko + Jupiter); copies publish RWA.xyz seed into staging
cargo run --bin rwa-audit -- run exchange --mode live

# Optional: refresh RWA.xyz SSR seed into staging (not artifacts/data/)
cargo run --bin rwa-exchange-freeze -- --live --refresh-rwa
# or: cargo run --bin rwa-audit -- freeze exchange --live --refresh-rwa
```

Manifest `audit_id` is `exchange-live-{date}`; claim `evidence_file` paths are relative to the staging directory (e.g. `data/exchange-live/gecko_aaplx_pools.json`).

Do not commit `data/exchange-live/` unless intentionally updating numbers after review.

## CI reproducibility

Exchange freeze uses `RWA_AUDIT_FROZEN_AT` when set (CI pins to committed `manifest.json` timestamp).

```bash
export RWA_AUDIT_FROZEN_AT="$(jq -r .frozen_at artifacts/data/manifest.json)"
cargo run --bin rwa-exchange-freeze
```

## Asset registry

Edit `config/assets/registry_v1.yaml` and `activity_v1.yaml` — no Rust change required for new assets.
