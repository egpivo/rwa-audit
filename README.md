# rwa-audit

Rust tooling to audit tokenized real-world assets (RWA). Extract on-chain evidence, score verifiable vs. claimed surfaces, surface gaps.

| Article | Post | Scope | Commands |
|---------|------|-------|----------|
| 1 | [If Everything Can Be Tokenized, What Should We Audit?](https://egpivo.github.io/2026/06/07/if-everything-can-be-tokenized-what-should-we-audit.html) | Contract-level registry and transfer metrics | `rwa-collect`, `rwa-activity` |
| 2 | [Where RWA Flow Leaves Traces](https://egpivo.github.io/2026/06/14/where-rwa-trades-and-exits-actually-clear.html) | Public pools (GeckoTerminal), aggregator quotes (ParaSwap), tx log reconstruction | `rwa-flow-panel`, `rwa-flow-quotes`, `rwa-flow-tx` |
| 3 | [Where RWA Exchange Risk Actually Sits](https://egpivo.github.io/2026/06/21/where-rwa-exchange-risk-actually-sits.html) | xStocks platform transfer, bridged value, Solana pool aggregates, Jupiter quote | `rwa-exchange-freeze` |

```bash
cargo run --bin rwa-collect
cargo run --bin rwa-activity
cargo run --bin rwa-flow-panel
cargo run --bin rwa-flow-quotes
cargo run --bin rwa-flow-tx -- 0x<tx_hash>
cargo run --bin rwa-exchange-freeze
cargo run --bin rwa-exchange-freeze -- --live
cargo test
```

Outputs: `data/flow/` (live collectors), `artifacts/data/` (frozen publish evidence). Figures: `artifacts/figures/`, `scripts/plot/`.

MIT — [LICENSE](LICENSE).
