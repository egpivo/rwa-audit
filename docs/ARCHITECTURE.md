# Architecture

See diagrams below. Implementation lives in `crates/rwa-audit/src/`.

## Layer diagram

```mermaid
flowchart TB
  subgraph CLI["CLI layer"]
    rwa_audit["rwa-audit run|freeze"]
    legacy["legacy bins: rwa-collect, rwa-freeze, …"]
  end

  subgraph Audit["audit/ — module contract"]
    trait["AuditModule trait"]
    modes["RunMode: live | frozen"]
    bundle["EvidenceBundle"]
    modules["modules: registry | activity | flow-* | exchange"]
  end

  subgraph Core["core/ — publish contract"]
    manifest["AuditManifest + claims"]
    promote["bundle promote → artifacts/audits/{id}/"]
  end

  subgraph Collectors["collectors"]
    collect["collect / activity"]
    flow["flow/ panel | quotes | tx_recon"]
    exchange["exchange/ freeze"]
  end

  subgraph Tools["tools/ — analysis primitives"]
    article1["registry: activity_surface | workflow_signature | sender_coverage | classify_surface"]
    article3["exchange: surface_compression | metric_equivalence"]
  end

  subgraph Sources["sources/ — adapters"]
    ctx["SourceContext"]
    transport["HttpTransport"]
    cache["ResponseCache → cache/sources/"]
    adapters["RPC | CoinGecko | Ethplorer"]
    prov["Provenance envelope on live JSON"]
  end

  subgraph Config["config/"]
    assets["assets/*.yaml"]
    sources_yml["sources.yaml"]
  end

  subgraph Storage["storage"]
    data["data/ Article 1 scratch"]
    exchange_live["data/exchange-live/ live probe"]
    artifacts["artifacts/data/ publish staging"]
    audits["artifacts/audits/{id}/ bundles"]
    fixtures["data/fixtures/ offline tests"]
  end

  subgraph Plot["scripts/plot/"]
    fig4["fig4_xstocks_surface.py"]
  end

  rwa_audit --> trait
  legacy --> Collectors
  trait --> modules
  modules --> Collectors
  modules --> Tools
  modules --> Core
  collect --> ctx
  flow -.-> transport
  exchange --> Core
  exchange --> prov
  ctx --> adapters
  adapters --> transport
  adapters --> cache
  modules --> assets
  Collectors --> data
  exchange --> artifacts
  exchange --> exchange_live
  promote --> audits
  fig4 --> audits
  fig4 --> artifacts
```

## Data flow (publish path)

```mermaid
sequenceDiagram
  participant U as Operator
  participant CLI as rwa-audit
  participant M as AuditModule
  participant S as Sources/APIs
  participant D as data/
  participant L as data/exchange-live/
  participant A as artifacts/data/
  participant B as artifacts/audits/

  U->>CLI: run registry --mode live
  CLI->>M: RegistryModule.run(live)
  M->>S: RPC, CoinGecko, Ethplorer
  S-->>M: cached responses
  M->>D: rwa_*.csv

  U->>CLI: run exchange --mode live
  CLI->>M: ExchangeModule.run(live)
  M->>L: manifest, panel, evidence JSON (no promote)

  U->>CLI: run exchange (frozen)
  CLI->>M: ExchangeModule.run(frozen)
  M->>A: manifest.json, panels, evidence JSON
  M->>B: atomic promote bundle + rewrite paths

  U->>CLI: fig4 script
  B-->>U: figures/xstocks_surface_snapshot.png
```

## Module → method → sources

| CLI module | AuditMethod | RunMode default | Source adapters |
|------------|-------------|-----------------|-----------------|
| `registry` | Registry | live | publicnode_rpc, coingecko, ethplorer |
| `activity` | Activity | live | publicnode_rpc |
| `flow-panel` | FlowSurface | live | geckoterminal*, yahoo_finance* |
| `flow-quotes` | FlowSurface | live | paraswap* |
| `flow-tx` | FlowSurface | live | publicnode_rpc |
| `exchange` | ExchangeSurface | frozen (live → `data/exchange-live/`) | manual_import, geckoterminal*, jupiter* |

\* Flow Gecko/ParaSwap/Jupiter still in `flow/` — migration to `sources/` pending.

## Directory layout

```
config/assets/          # asset universe (YAML)
config/sources.yaml     # source registry
crates/rwa-audit/src/
  audit/                # Phase 4 unified CLI + AuditModule
  core/                 # manifest + bundle promote
  sources/              # Phase 3 adapters + cache
  evm.rs                # hex / log parsing
  collect.rs activity.rs flow/ exchange/
  tools/                # article 1 + 3 analysis primitives
data/                   # Article 1 live scratch (gitignored)
data/exchange-live/     # Article 3 live exchange probe (gitignored; not promoted)
data/flow/              # committed flow snapshots
data/fixtures/          # adapter test fixtures
artifacts/data/         # flat frozen evidence
artifacts/audits/       # versioned publish bundles
cache/sources/          # API response cache (gitignored)
scripts/plot/           # figure generators
```
