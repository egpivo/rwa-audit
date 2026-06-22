# RWA Data Quality Notes

Generated: 2026-06-02T01:54:17.354288+00:00

## Data Sources Used

- **On-chain logs (Ethereum)**: mevblocker.io public RPC for PAXG/XAUT; ethereum.publicnode.com for all other Ethereum assets
- **On-chain logs (Polygon)**: polygon-bor-rpc.publicnode.com (severely pruned; only ~50 hours of history available)
- **Token info and holder data**: Ethplorer API (freekey, Ethereum only)
- **Prices**: CoinGecko free API; hardcoded approximations for BUIDL ($1.00), BENJI ($1.00), PAXG (~$3,280/oz), XAUT (~$3,265/oz)
- **Contract addresses**: Etherscan / official issuer documentation verified via web search
- **Etherscan API**: V1 API deprecated (NOTOK responses); V2 requires API key not available on free tier

## API Limitations Encountered

### Etherscan API Deprecation
The Etherscan V1 API (api.etherscan.io/api) returned "NOTOK: You are using a deprecated V1 endpoint" for all calls. V2 (api.etherscan.io/v2/api) requires a registered API key. As a result:
- Transfer logs were obtained from public JSON-RPC nodes instead
- Holder counts were obtained from Ethplorer (freekey tier) instead of Etherscan
- ABI-based permissioning checks were replaced with documentation-based assessment

### publicnode.com Rate Limits
- Ethereum node: getLogs limited to 40,000 blocks per call; frequent timeouts on first call in sequence
- Polygon node: History pruned to approximately 50 hours; 6-month history unavailable

### mevblocker.io Limits
- getLogs returns at most 10,000 results per call, with error-hint-based adaptive range negotiation
- Used for PAXG and XAUT monthly data collection; required 10-16 calls per month to traverse one month

### CoinGecko Rate Limiting
- Free tier experienced HTTP 429 errors for some tokens (MPL, TRU) during initial collection run
- Retry logic with exponential backoff partially resolved this

## General Caveats

- Transfer logs cover last 6 months of on-chain history (December 2025 – June 2, 2026)
- USD approximations use CoinGecko spot prices at collection time; do not reflect official NAV
- Permissioning status is based on public documentation, not live contract ABI inspection
- Ethplorer holder data may lag on-chain state by hours; free tier rate-limited
- BENJI (Franklin Templeton) Polygon data only; the canonical record is on Stellar
- MPL and TRU are governance tokens, not direct RWA instruments; included as proxies for private credit exposure
- PAXG and XAUT transfer counts for months Feb–May 2026 are extrapolated from 10,000-log samples; scale factors ranged from 12x to 85x

## Per-Asset Notes

### Franklin Templeton BENJI (FOBXX) (BENJI) — Polygon

**Data Issues Encountered:**
- Ethplorer does not support Polygon; holder data unavailable.
- Top holder data not available from Ethplorer.
- No transfer logs found in last 6 months.

**Context:**
- Franklin OnChain U.S. Government Money Fund; Polygon deployment; originally on Stellar
- Price USD used: 1.0 (hardcoded approximate).
- Permissioned (public docs): True.

### Paxos Gold (PAXG) — Ethereum

**Data Issues Encountered:**
- No transfer logs found in last 6 months.

**Context:**
- 1 PAXG = 1 troy oz gold; stored in Brinks vaults
- Permissioned (public docs): False.

## Asset-Specific Structural Notes

### Franklin Templeton BENJI (FOBXX)
- Primary blockchain record is on Stellar. The Polygon deployment is secondary.
- Polygon contract may show limited activity relative to Stellar.
- The fund is a SEC-registered money market mutual fund, not a DeFi protocol.

### Ondo OUSG
- Restricted to US institutional investors with KYC/AML. Very low transfer count is expected.
- Official NAV updated daily; no continuous secondary market pricing.

### Ondo USDY
- Accessible to non-US investors. More liquid than OUSG.
- Price appreciation model: NAV increases over time rather than rebasing.

### BlackRock BUIDL
- Whitelist-controlled; only approved institutional investors can transact.
- Available on multiple chains (Ethereum, Arbitrum, Optimism, Avalanche). This study covers Ethereum only.
- As of early 2025, BUIDL was the largest tokenized treasury fund by AUM.

### Superstate USTB
- Uses a separate PermissionList contract to gate transfers.
- NAV oracle updates continuously (second-by-second).

### Paxos Gold (PAXG)
- Most liquid tokenized gold on Ethereum with deep DEX activity.
- 1:1 backed by LBMA gold bars; Paxos is the custodian.

### Tether Gold (XAUT)
- Issued by Tether; 1 XAUT = 1 troy oz on LBMA bars.
- Less DeFi integration than PAXG.

### Maple Finance MPL
- Governance token, not a direct loan/credit instrument.
- Protocol migration toward Syrup token (SYRUP) underway as of 2024.

### TrueFi TRU
- Governance/staking token for the TrueFi lending protocol.
- TrueFi TVL has declined significantly since 2022 peak.

### Backed bIB01 / bCSPX
- Issued under Swiss DLT law; secondary transfers unrestricted.
- Minting/redemption only through Backed platform with KYC.
- Low trading volumes; primarily held by institutional DeFi protocols.
