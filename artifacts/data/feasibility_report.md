# RWA On-Chain Feasibility Study
## Tokenized Real World Assets: Data Availability, Transfer Patterns, and Analytical Constraints

**Date:** June 2, 2026
**Data Coverage:** December 2025 – June 2, 2026 (approximately 6 months)
**Assets Studied:** 11 contracts across 5 categories (Ethereum and Polygon)
**Data Sources:** publicnode.com RPC, mevblocker.io RPC, Ethplorer API (freekey), CoinGecko API (free tier)

---

## 1. Executive Summary

This feasibility study collects and analyzes publicly available on-chain data for eleven tokenized real-world asset (RWA) instruments spanning five categories: tokenized treasuries and money market funds, gold-backed tokens, private credit governance tokens, and tokenized ETFs. All data is sourced from free-tier public APIs — no proprietary data feeds or paid endpoints were required for the core analysis.

The study confirms that on-chain analysis of RWA tokens is feasible for several key signals: transfer volume and frequency, holder concentration, mint and burn activity, and the presence of permissioning mechanisms. However, structural limitations substantially constrain what can be observed. Permissioned RWA tokens (OUSG, BUIDL, USTB) deliberately suppress secondary transfer activity; the on-chain record reflects authorized institutional flows only and systematically underrepresents the full picture. The most liquid tokens (PAXG, XAUT) generate transfer volumes so large that free-tier APIs can only sample a fraction of on-chain activity. Tokens on non-Ethereum chains face data availability constraints: the Polygon BENJI deployment has pruned history and shows near-zero Polygon-side activity, consistent with Franklin Templeton's canonical ledger residing on Stellar rather than any EVM chain.

**Key finding:** On-chain data is most informative for open tokens (PAXG, USDY, XAUT) and least informative precisely for the most institutionally significant products (BUIDL, OUSG), where permissioning creates a selection bias in who can transact. A complete RWA market analysis requires combining on-chain signals with off-chain data (official NAV filings, AUM disclosures, prospectus documents).

---

## 2. Asset Coverage and Data Availability

### 2.1 Asset Registry Summary

| Symbol | Category | Chain | Total Supply | Approx. USD | Permissioned | Holders |
|--------|----------|-------|-------------|-------------|--------------|---------|
| BENJI | Tokenized Treasury/MMF | Polygon | 31,848,811 | ~$31.8M | Yes | N/A |
| OUSG | Tokenized Treasury/MMF | Ethereum | 2,125,391 | N/A | Yes | 50 |
| USDY | Tokenized Treasury/MMF | Ethereum | 968,583,303 | ~$1.09B | Partial | 913 |
| BUIDL | Tokenized Treasury/MMF | Ethereum | 178,544,793 | ~$178.5M | Yes | 59 |
| USTB | Tokenized Treasury/MMF | Ethereum | 65,401,304 | N/A | Yes | 78 |
| PAXG | Gold/Commodities | Ethereum | 464,217 troy oz | ~$1.52B | No | 89,550 |
| XAUT | Gold/Commodities | Ethereum | 707,747 troy oz | ~$2.31B | No | 53,929 |
| MPL | Private Credit | Ethereum | 10,000,000 | ~$4.1M | No | 8,486 |
| TRU | Private Credit | Ethereum | 1,257,020,975 | ~$1.1M | No | 15,911 |
| bIB01 | Tokenized Fund/ETF | Ethereum | 45,327 | N/A | No | 24 |
| bCSPX | Tokenized Fund/ETF | Ethereum | 2,843 | N/A | No | 76 |

**Notes:**
- BENJI supply and USD approximation use $1.00 per share (money market). Canonical record is on Stellar.
- OUSG and USTB NAV-based pricing not available via public APIs.
- PAXG and XAUT USD values use spot gold prices at time of data collection (~$3,280/oz and $3,265/oz respectively).
- bIB01 and bCSPX USD NAV prices not available via free-tier public APIs.
- TRU decimal precision: the contract uses 8 decimals; this affects reported volume figures (see Section 4).

### 2.2 Data Availability by Asset

| Symbol | Transfer Logs | Holder Count | Top Holders | Mint/Burn | Price (USD) |
|--------|--------------|-------------|------------|-----------|------------|
| BENJI | Partial (2 days, Polygon) | No | No | Yes | Hardcoded |
| OUSG | Full 6 months | Yes (50) | Yes | Yes | No |
| USDY | Full 6 months | Yes (913) | Yes | Yes | Yes |
| BUIDL | Full 6 months | Yes (59) | Yes | Yes | Yes |
| USTB | Full 6 months | Yes (78) | Yes | Yes | No |
| PAXG | Full 6 months (sampled) | Yes (89,550) | Yes | Partial | Yes |
| XAUT | Full 6 months (sampled) | Yes (53,929) | Yes | Partial | Yes |
| MPL | Full 6 months | Yes (8,486) | Yes | No mints observed | Yes |
| TRU | Full 6 months | Yes (15,911) | Yes | Sparse | Yes |
| bIB01 | Partial (4 months) | Yes (24) | Yes | None observed | No |
| bCSPX | Full 6 months | Yes (76) | Yes | None observed | No |

---

## 3. Key Patterns Observed

### 3.1 Transfer Activity

**Tokenized Treasuries (BUIDL, USTB, OUSG)**

BUIDL showed the most stable institutional transfer pattern among permissioned tokens: roughly 550–740 transfers per month, nearly all driven by mints from and to the zero address. With only 7–16 unique senders per month, the wallet set is tightly controlled. Monthly settlement volume ranged from $14M to $65M, consistent with an institutional-grade product where each "transfer" typically represents an authorized subscription, redemption, or intra-protocol settlement rather than a retail trade.

USTB showed even higher transfer frequency (998–2,181 per month), with 30–47 unique participants — notably more than BUIDL. Net issuance fluctuated between positive and negative month-to-month, indicating active in-and-out flows. The higher transaction count relative to BUIDL likely reflects Superstate's architecture, where yield accrual events generate frequent protocol-internal transfers rather than user-driven trading.

OUSG had the fewest transfers (34–91/month), which aligns with its strict investor qualification requirements. Net issuance has been persistently negative from January 2026 onward, suggesting sustained redemption pressure or portfolio rebalancing away from OUSG toward other products (possibly USDY or other instruments).

**USDY: The Breakout Product**

USDY is the most striking data point in this study. Monthly transfer counts grew from 343 in December 2025 to 1,930 in May 2026, a 5.6x increase in six months. Monthly volume reached 1.41 billion tokens in May 2026 (approximately $1.6B USD equivalent at current NAV). With ~250 unique senders and ~350 unique receivers per month, USDY has a far larger active wallet set than BUIDL or OUSG.

The mint/burn data reveals high volatility in net issuance: a massive net mint of ~388M tokens in March 2026 and ~405M tokens in May 2026, interspersed with large net redemptions. This pattern is consistent with leveraged DeFi strategies that cycle in and out of USDY for yield, rather than pure buy-and-hold accumulation.

**Gold Tokens (PAXG, XAUT)**

PAXG and XAUT are by far the most actively traded assets in this study. Transfer counts are estimated in the range of 120,000–870,000 per month (extrapolated from 10,000-log samples). These extrapolations carry high uncertainty (scale factors of 12x–85x), but the directional signal is clear: gold-backed tokens are order-of-magnitude more liquid than treasury tokens. PAXG appears to have higher peak activity than XAUT, possibly due to deeper DeFi integration (Uniswap, Curve pools).

Gold token mint/burn activity was sparse in our samples, suggesting Paxos and Tether do not frequently create or destroy tokens in secondary market periods — the circulating supply is relatively stable, and price exposure is provided by secondary trading rather than new issuance.

**Private Credit Governance Tokens (MPL, TRU)**

MPL had modest, stable transfer activity (193–333/month) with entirely zero mint/burn events — the MPL token supply is fixed and no new minting or burning occurred in the study period. This is consistent with a governance token in maintenance mode, with Maple Finance having transitioned toward the SYRUP token.

TRU showed an April 2026 spike to 23,725 transfers and ~$8.4M volume — a 6x jump above baseline — which may reflect a protocol event, token distribution, or governance action. A single large burn transaction (193M TRU tokens) occurred in May 2026, which could represent a token buyback, protocol treasury adjustment, or whale exit.

**Backed Finance Tokens (bIB01, bCSPX)**

Both Backed tokens have extremely low on-chain activity (1–211 transfers per month) and very few holders (24 and 76 respectively). Top-1 concentration is 99.87% for bIB01 and 99.61% for bCSPX — nearly all circulating supply sits in a single address, strongly suggesting these tokens are primarily used as collateral in DeFi protocols rather than actively traded. This is consistent with their positioning as building blocks for on-chain structured products.

### 3.2 Holder Concentration

The contrast in holder concentration between open tokens and permissioned tokens is sharp:

| Category | Representative | Holders | Top-10 Concentration | Top-1 Concentration |
|----------|---------------|---------|---------------------|---------------------|
| Gold (open) | PAXG | 89,550 | 34.7% | 20.4% |
| Gold (open) | XAUT | 53,929 | 54.6% | 13.4% |
| Treasury (permissioned) | BUIDL | 59 | 82.8% | 30.4% |
| Treasury (permissioned) | OUSG | 50 | 93.3% | 30.8% |
| Treasury (permissioned) | USTB | 78 | 85.3% | 19.6% |
| Treasury (semi-open) | USDY | 913 | 96.5% | 37.1% |
| Tokenized ETF | bCSPX | 76 | 99.8% | 99.6% |

PAXG has the most distributed holder base — 89,550 holders with top-10 holding only 34.7% of supply — consistent with a retail-accessible commodity token. By contrast, every permissioned treasury token has fewer than 100 holders and top-10 concentration above 80%. This structural feature means that for permissioned RWA tokens, "holder count" is a measure of authorized institutional relationships rather than market breadth.

USDY's high top-10 concentration (96.5% despite 913 holders) points to a small number of large DeFi protocols dominating holdings, which is consistent with USDY's use as yield-bearing collateral.

### 3.3 Mint and Burn Activity

**BUIDL:** Regular mint/burn cadence, averaging 575 mints and 4 burns per month. The very low burn count relative to mint count may indicate that most BUIDL "redemptions" occur via peer-to-peer transfers to the issuer's smart contract rather than classical ERC-20 burns. Net issuance turned sharply positive in May 2026 (+$30.9M), suggesting significant new institutional inflows.

**USTB:** Unusually high burn counts (319–780 per month) relative to mint counts (129–222). This pattern is distinct from BUIDL and may reflect USTB's yield distribution mechanism (yield accrual burns and re-mints tokens to reflect NAV appreciation) rather than pure redemptions.

**OUSG:** Consistently negative net issuance from January 2026 onward, totaling approximately -4.2M OUSG tokens net redeemed over the period. The shrinkage is gradual rather than a single large exit.

**USDY:** Highly volatile net issuance. Large positive net mints in March and May 2026 (+388M, +405M tokens) offset by large net redemptions in January, February, and April. This volatility is inconsistent with a simple buy-and-hold product and strongly suggests leveraged cycling behavior.

---

## 4. Permissioning and Transfer Restriction Observations

Five of eleven assets exhibit confirmed on-chain permissioning:

| Symbol | Permissioning Mechanism | Evidence |
|--------|------------------------|----------|
| OUSG | Allowlist on token contract | Known from public documentation; low holder count |
| BUIDL | Whitelist controlled by Securitize | Known from public documentation; 59 holders |
| USTB | Separate PermissionList contract | Known from Superstate docs; 78 holders |
| BENJI | Stellar-side KYC; Polygon bridge | Very low Polygon activity despite $31.8M fund size |
| USDY | Geographic restrictions | More open than others; 913 holders; labeled "partial" |

From an analysis standpoint, permissioning creates a systematic observability problem: the contracts that represent the largest institutional AUM (BUIDL was the largest tokenized treasury fund as of early 2025) generate the least informative secondary transfer data, because transfers only occur between a handful of vetted counterparties. Observing 59 holders of BUIDL tells us nothing about the fund's $178M AUM being spread across millions of underlying investors — the blockchain layer provides custody atomicity, not economic distribution.

This is not a data quality problem but a fundamental design choice: these instruments use the blockchain as a settlement and custody rail, not as a price discovery or liquidity venue.

---

## 5. Limitations of On-Chain Data for RWA Analysis

### 5.1 Permissioning Creates Observability Gaps

As noted above, permissioned tokens suppress the very signals — secondary market liquidity, organic price discovery, holder distribution — that on-chain analysis is best suited to capture. For OUSG with 50 holders, each transfer event is meaningful; for BUIDL, the transfers between authorized wallets are settlement mechanics, not market signals.

### 5.2 NAV vs. On-Chain Price

None of the treasury tokens (BENJI, OUSG, USTB) have liquid on-chain price discovery. Their true economic value is the NAV filed daily or continuously by the fund manager. Public APIs cannot retrieve NAV; it requires access to official fund disclosures. USD approximations in this study are either hardcoded ($1.00 for BUIDL, BENJI) or unavailable (OUSG, USTB, bIB01, bCSPX).

### 5.3 Transfer Volume vs. Economic Activity

High transfer counts for USDY, PAXG, and XAUT do not necessarily represent new investment. A large fraction of transfers are:
- DeFi protocol internal accounting (rebases, routing, liquidations)
- Wrapping and unwrapping across bridges
- Collateral movements in lending protocols

Volume figures in this study measure gross token movement, not net economic inflows or outflows.

### 5.4 PAXG/XAUT Sampling Uncertainty

Because PAXG and XAUT generate more than 10,000 transfer events in a typical block range, our monthly metrics are extrapolated from 10,000-log samples. Scale factors ranged from 12x to 85x. The extrapolated transfer counts (200,000–870,000/month for PAXG) should be treated as order-of-magnitude estimates, not precise counts. Mint/burn data for PAXG in months Feb–May 2026 is entirely absent because our samples happened to not include zero-address transfers; this does not mean no minting occurred.

### 5.5 Cross-Chain Fragmentation

BUIDL exists on at least four chains (Ethereum, Arbitrum, Optimism, Avalanche). USDY has Ethereum and Arbitrum deployments. BENJI spans Stellar, Polygon, Avalanche, Base, Ethereum, and others. This study covers only the primary Ethereum contract for each and one Polygon contract (BENJI). Aggregate AUM and transfer volume across all chains is substantially higher than the Ethereum-only figures reported here.

### 5.6 BENJI Data Void

The Polygon BENJI contract showed only 3 transfers over the final 50 hours of available Polygon history. The Polygon node's history had been pruned beyond approximately 50 hours. This is not a reflection of BENJI's actual activity; the canonical fund ledger is on Stellar, where the study has no API access. The Polygon figures in this report should be considered illustrative of the deployment's existence, not its activity level.

### 5.7 Etherscan V1 API Deprecation

During data collection, all Etherscan V1 API endpoints returned deprecation errors ("switch to Etherscan API V2"). V2 requires an API key not available at free tier. As a result, holder count data was sourced from Ethplorer (freekey tier) instead of Etherscan. Transfer logs were obtained directly from public JSON-RPC nodes. This did not materially affect data completeness but may introduce minor discrepancies in holder counts relative to Etherscan's index.

---

## 6. Recommendations for Deeper Study

**6.1 Integrate Official Fund Filings**
Pair on-chain transfer data with official AUM disclosures (SEC filings for FOBXX, fund factsheets for BUIDL, Ondo/Superstate public dashboards) to link blockchain activity to actual economic scale. On-chain data alone cannot determine how much money is in these funds.

**6.2 Multi-Chain Aggregation**
Build a cross-chain aggregator for BUIDL, USDY, and BENJI to capture the full transfer picture across Ethereum, Arbitrum, Optimism, Avalanche, Polygon, Base, and Stellar. A significant fraction of RWA activity is happening on L2s where gas costs are lower and DeFi integrations more active.

**6.3 Counterparty Graph Analysis**
For permissioned tokens, the small wallet set allows full counterparty graph construction. Mapping which wallets interact with each other (e.g., which BUIDL holders are also USDY holders or USTB holders) would reveal the institutional cohort driving RWA adoption and overlap between products.

**6.4 Yield Accrual vs. Inflow Analysis**
USDY and USTB both appear to use mint/burn events for yield distribution mechanics. Building a methodology to distinguish "new investor subscription mint" from "yield distribution mint" would significantly improve the interpretability of net issuance figures.

**6.5 DeFi Integration Depth**
Identify the top 5–10 DeFi protocols holding each RWA token (Aave, Morpho, Pendle, Spark, etc.) and track the fraction of supply locked in these protocols over time. For USDY and bCSPX, near-total concentration in smart contract addresses suggests DeFi-as-primary-use-case rather than direct institutional holding.

**6.6 Gold Token Mint/Burn Audit**
PAXG and XAUT mint/burn data was incomplete in this study due to sampling. A targeted analysis of zero-address transfers only (much lower volume than regular transfers) would give a cleaner picture of gold supply issuance and redemption patterns without hitting log volume caps.

**6.7 Time-Series AUM Reconstruction**
Using the full historical Transfer log (not just 6 months), reconstruct the outstanding supply of each token at each block, creating a time-series of AUM. This would show when each product launched, how quickly it grew, and whether there have been sharp drawdown events.

---

## 7. Summary of Collected Data

- **11 assets** studied across Ethereum (10) and Polygon (1)
- **65 monthly observations** in rwa_transfer_metrics.csv (covering Dec 2025 – Jun 2026)
- **65 monthly observations** in rwa_mint_burn_metrics.csv
- **11 holder metrics rows** in rwa_holder_metrics.csv
- Transfer data is complete for OUSG, USDY, BUIDL, USTB, MPL, TRU, bCSPX (6 months full coverage)
- Transfer data is sampled/extrapolated for PAXG and XAUT (10,000-log samples per month)
- Transfer data is severely limited for BENJI (Polygon history pruned) and partial for bIB01
- Total on-chain logs processed: approximately 340,000 raw Transfer events before sampling/deduplication
- All output files are in /Users/joseph/agentic-research/data/

---

*This report describes only what can be observed from public on-chain data and public documentation. It makes no claims about NAV accuracy, reserve backing, legal ownership rights, redemption guarantees, or investment suitability. USD approximations are illustrative only.*
