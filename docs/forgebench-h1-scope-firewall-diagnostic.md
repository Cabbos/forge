# ForgeBench H1 scope-firewall diagnostic

This development-only sequence isolates two concrete scope failure classes
observed in paid Forge runs: project `uv` commands generating `uv.lock`, and
models editing adjacent implementation files outside the case allowlist. It is
not a paired causal A/B comparison because every campaign uses a different
source commit. No holdout was opened and no frontier-quality claim is made.

## Results

| Campaign | Change under test | Trusted success | Verification | Scope violations | Median rounds | Total CNY |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `h1-scopefirewall-e289-v3` | helper-file prompt only | 0.8333 | 0.9667 | 0.1333 | 16 | 0.71730156 |
| `h1-uvfirewall-f146` | runtime `uv` scope shim | 0.9667 | 1.0000 | 0.0333 | 16 | 0.64706196 |
| `h1-scopehard-7746` | shim + exact allowlist hard boundary | 1.0000 | 1.0000 | **0.0000** | 15 | 0.63038252 |
| `h1-roundbudget-34a8` | temporary phase-budget prompt | 0.9667 | 0.9667 | 0.0000 | 16 | 0.67849560 |

The runtime shim removed the repeated `uv.lock` false-positive class. The hard
allowlist wording then eliminated the remaining adjacent-file violation in the
30-attempt development campaign, reaching 30/30 trusted success, verification,
and scope compliance. This is strong development evidence for the scope
firewall, but it is still not a holdout result.

The phase-budget prompt did not improve convergence: median rounds stayed at 16
and trusted/verification success fell to 29/30, so it was reverted. The current
stack is therefore the runtime `uv` scope wrapper plus the exact allowlist hard
boundary; the round gate remains open because the best observed median is 15,
above the locked ≤12 target.

The secret-free machine-readable record, including report and artifact-index
hashes, is [`h1-scope-firewall-summary.json`](../apps/eval-runner/forgebench/results/h1-scope-firewall-summary.json).
Raw campaign roots remain outside Git under `/private/tmp`; no provider text,
credential, or key material is published.
