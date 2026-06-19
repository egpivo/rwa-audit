# Data sources

Registry: `config/sources.yaml`. Runtime cache: `cache/sources/{source_id}/`.

| Source | Adapter | Used by |
|--------|---------|---------|
| publicnode_rpc | `PublicNodeRpcAdapter` | collect, activity, tx_recon |
| coingecko | `CoinGeckoAdapter` | collect |
| ethplorer | `EthplorerAdapter` | collect (Ethereum holders) |
| geckoterminal | flow `GeckoClient` (pending migration) | flow panel, exchange |
| paraswap | flow (pending) | flow quotes |
| jupiter | flow (pending) | exchange freeze `--live` |

Offline fixtures for adapter tests: `data/fixtures/`.

Set `ETHPLORER_API_KEY` for production Ethplorer quota. API responses are cached under `cache/` (gitignored).
