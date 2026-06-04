# rwa-audit

Rust tooling to audit tokenized real-world assets (RWA). Long-term: extract on-chain evidence, score verifiable vs. claimed surfaces, surface gaps. **Today:** evidence collection only.

```bash
cargo run --bin rwa-collect    # registry + monthly metrics → data/
cargo run --bin rwa-activity   # 30-day daily activity → data/
cargo test
```

No API keys (public RPC + Ethplorer free tier). Figures: `scripts/plot/`. Drafts: `article/`.

MIT — [LICENSE](LICENSE).
