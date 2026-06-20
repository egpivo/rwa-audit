# Publish workflow

Frozen audit bundles are the publish contract. Live collectors write scratch data; freeze promotes evidence into `artifacts/audits/{audit-id}/`.

## Article 1 — Registry + activity

```bash
cargo run --bin rwa-audit -- run article1 --promote
```

Runs registry collection + activity timeseries atomically under a single exclusive lock, then promotes the evidence. The manifest `as_of` / `panel_date` is derived from the on-chain block timestamp of the latest collected block — never from the wall clock or `RWA_AUDIT_FROZEN_AT`. `RWA_AUDIT_FROZEN_AT` only pins the manifest `frozen_at` field (when the promotion ran).

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

Exchange `panel_date` is controlled by `PUBLISH_PANEL_DATE` (offline freeze fixture date) or defaults to today's date for live runs. `RWA_AUDIT_FROZEN_AT` only pins the manifest `frozen_at` field (when the promotion ran) — it does not affect `panel_date` for any module.

Article 1 `panel_date` and `as_of` always come from the on-chain block timestamp of the latest collected block — neither `RWA_AUDIT_FROZEN_AT` nor any other env var can override them.

```bash
export RWA_AUDIT_FROZEN_AT="$(jq -r .frozen_at artifacts/audits/article3-xstocks-2026-06-12/manifest.json)"
rwa-audit freeze exchange
```

## Asset registry

Edit `config/assets/registry_v1.yaml` and `activity_v1.yaml` — no Rust change required for new assets.
