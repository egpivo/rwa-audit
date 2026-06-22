# Data sources

Registry: `config/sources.yaml` (required at runtime via `SourceRegistry::load_default()`).
Runtime cache: `{cache.root}/sources/{source_id}/` (default `cache/sources/`). Live collectors use `SourceContext::for_live_collection()` (5-minute TTL).

Missing or unknown source entries fail at resolve time; adapters read `base_url` from the registry (no hardcoded URL fallbacks).

## Architecture

```
config/sources.yaml  →  SourceRegistry  →  SourceAdapter  →  HttpTransport
                              ↑
                        SourceContext::fetch(SourceId, SourceRequest)
```

Each adapter implements wire-level `fetch` plus typed helpers (e.g. `GeckoTerminalAdapter::token_pools`).
HTTP failures are not cached; provenance records the full request URL including query parameters.
GeckoTerminal retries with long backoff only on HTTP 429; other HTTP errors fail immediately.

## Adapters

| Source | Adapter | Used by |
|--------|---------|---------|
| publicnode_rpc | `PublicNodeRpcAdapter` | collect, activity, tx_recon |
| coingecko | `CoinGeckoAdapter` | collect |
| ethplorer | `EthplorerAdapter` | collect (Ethereum holders) |
| geckoterminal | `GeckoTerminalAdapter` | flow panel, exchange freeze |
| paraswap | `ParaSwapAdapter` | flow quotes |
| jupiter | `JupiterAdapter` | exchange freeze `--live` |
| yahoo_finance | `YahooFinanceAdapter` | flow panel (GC=F reference) |
| rwa_xyz | `http_get_text_cached` (HTML scrape) | exchange RWA.xyz seed refresh |
| manual_import | (file; no HTTP adapter) | exchange offline fixtures |

## Cache config

`config/sources.yaml` `cache.enabled` and `cache.root` are applied when constructing `SourceContext` (`SourceRegistry::build_cache()`).

## Adding a source

1. Add entry to `config/sources.yaml` (`kind`, `base_url` or RPC endpoints, `rate_limit_ms`)
2. Add `SourceId` variant in `sources/types.rs`
3. Implement `SourceAdapter` in `sources/adapters/{name}.rs`
4. Register in `sources/registry.rs::resolve_adapter_impl` (profile must exist in yaml)
5. Wire `AuditModule::required_sources()` if applicable
6. Add fixture JSON under `data/fixtures/` for offline tests

Offline fixtures for adapter tests: `data/fixtures/`.

Set `ETHPLORER_API_KEY` for production Ethplorer quota. API responses are cached under `cache/` (gitignored).
