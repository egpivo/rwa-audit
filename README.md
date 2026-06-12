# rwa-audit

Rust tooling to audit tokenized real-world assets (RWA). Extract on-chain evidence, score verifiable vs. claimed surfaces, surface gaps.

| Article | Post | Scope | Commands |
|---------|------|-------|----------|
| 1 | [If Everything Can Be Tokenized, What Should We Audit?](https://egpivo.github.io/2026/06/07/if-everything-can-be-tokenized-what-should-we-audit.html) | Contract-level registry and transfer metrics | `rwa-collect`, `rwa-activity` |
| 2 | [Where RWA Flow Leaves Traces](https://egpivo.github.io/2026/06/14/where-rwa-trades-and-exits-actually-clear.html) | Public pools (GeckoTerminal), aggregator quotes (ParaSwap), tx log reconstruction | `rwa-flow-panel`, `rwa-flow-quotes`, `rwa-flow-tx` |

```bash
cargo run --bin rwa-collect
cargo run --bin rwa-activity
cargo run --bin rwa-flow-panel      # 90d panel 2026-03-10 → 2026-06-08
cargo run --bin rwa-flow-quotes
cargo run --bin rwa-flow-tx -- 0x<tx_hash>
cargo test
```

Outputs: `data/` and `data/flow/`. Figures: `scripts/plot/` (see `article/writing/draft_v2_flow_traces.md`).

MIT — [LICENSE](LICENSE).
