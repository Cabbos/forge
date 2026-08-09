# Kimi K3 trusted development diagnostic — 2026-08-10

This is the first complete Kimi development run after the ForgeBench golden
trust self-check correction. It is development-only evidence: it is not a
paired causal comparison, a sealed holdout result, or a frontier-quality
claim.

## Identity and boundary

- Explicit chain: `kimi-k3-deepseek`.
- Primary: Kimi `k3`, `custom_openai`, OpenAI Chat Completions at
  `https://api.kimi.com/coding/v1`.
- Fallback: DeepSeek V4 Flash only for the narrow authentication/quota/rate
  limit boundary; no fallback event occurred.
- Cases: all 10 canonical development cases, one attempt each.
- Gate: `--require-known-cost --max-total-cost-usd 2.50`.
- Credentials: read from CCSwitch into process environment variables only; no
  secret value was written to the artifact or repository.

## Aggregate result

| Metric | Result |
| --- | ---: |
| tasks | 10 / 10 |
| success | 1.0000 |
| verification | 1.0000 |
| scope violations | 0 / 10 |
| average model rounds | 10.4 |
| average confirmation requests | 4.7 |
| input tokens | 274,913 |
| output tokens | 16,789 |
| provider usage known | 1.0000 |
| provider cost known | 1.0000 |
| total cost | `$0.468318` |
| trust result | `trusted: true` |
| trust blockers | 0 |
| fallback events | 0 |

All ten trace identity events resolved to the Kimi route, so this report is
not a mixed-provider result. Every task passed controller verification and
stayed within its declared file contract; the no-op task changed nothing.

The earlier post-write-policy Kimi run had 11.6 average rounds and `$0.5847462`
cost but was polluted by the now-fixed golden self-check. The new 10.4-round
sample is useful as a trusted baseline for further experiments, but one
development run is insufficient to attribute a material intelligence gain.

Artifact: `/tmp/forge-kimi-trusted-development-2026-08-10.json` (raw local
diagnostic; outside Git).
