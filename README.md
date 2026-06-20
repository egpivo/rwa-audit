# rwa-audit

Rust tooling to audit tokenized real-world assets (RWA). Extract on-chain evidence, score verifiable vs. claimed surfaces, surface gaps.

| Article | Post | Scope | Unified module |
|---------|------|-------|----------------|
| 1 | [If Everything Can Be Tokenized, What Should We Audit?](https://egpivo.github.io/2026/06/07/if-everything-can-be-tokenized-what-should-we-audit.html) | Contract registry + activity | `article1` |
| 2 | [Where RWA Flow Leaves Traces](https://egpivo.github.io/2026/06/14/where-rwa-trades-and-exits-actually-clear.html) | Pools, quotes, tx recon | `flow-panel`, `flow-quotes`, `flow-tx` |
| 3 | [Where RWA Exchange Risk Actually Sits](https://egpivo.github.io/2026/06/21/where-rwa-exchange-risk-actually-sits.html) | xStocks exchange surface | `exchange` |

## Unified CLI

```bash
cargo run --bin rwa-audit -- run article1 --promote     # registry + activity → artifacts/audits/
cargo run --bin rwa-audit -- run flow-panel --mode live
cargo run --bin rwa-audit -- run flow-quotes --mode live
cargo run --bin rwa-audit -- run flow-tx 0x<tx_hash>
cargo run --bin rwa-audit -- run exchange              # frozen → artifacts/data/ + promote
cargo run --bin rwa-audit -- run exchange --mode live  # probe → data/exchange-live/ only
cargo run --bin rwa-audit -- freeze list
cargo run --bin rwa-audit -- freeze promote article3-xstocks-2026-06-12
cargo test
```

Legacy binaries (`rwa-collect`, `rwa-freeze`, …) still work. Makefile: `make sync`, `make test`, `make freeze`.

Docs: [docs/PUBLISH.md](docs/PUBLISH.md) · [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) · `config/assets/*.yaml` · [docs/sources/README.md](docs/sources/README.md)

**Sync with remote:**

```bash
make sync
```

Outputs: `data/` (Article 1 live scratch), `data/exchange-live/` (Article 3 live exchange probe; gitignored), `data/flow/` (flow snapshots), `artifacts/data/` (frozen publish staging), `artifacts/audits/{id}/` (publish bundles), `cache/sources/` (API cache).

MIT — [LICENSE](LICENSE).
