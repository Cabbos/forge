# Forge eval provider fallback

The development/ordinary eval runner now supports an explicit provider chain
through `ProviderFallbackRunner`. A primary Forge runner can use a provider such
as Kimi `k3` through its OpenAI-compatible endpoint, while a separately
configured DeepSeek runner can take over when the primary provider returns an
authentication or capacity response.

The fallback boundary is deliberately narrow:

- `401`, `403`, `429`, rate-limit, quota, and equivalent authentication markers
  trigger one clean fallback attempt;
- verification failures, scope violations, tool failures, and model-budget
  failures do not trigger a provider switch;
- each runner owns a fresh isolated workspace and runtime HOME;
- the emitted `eval_provider_fallback` event contains only bounded provider,
  model, and reason-code identity, never raw provider text or credentials.

This is a development/ordinary-run capability. The locked DeepSeek H1/H2
profiles and holdout route contract remain unchanged; fallback traces must not
be mixed into a single-provider A/B or frontier claim. Provider keys are
supplied through child-process environment overrides and are not persisted in
profiles, artifacts, logs, or this repository.

The checked-in ordinary-run entry point is explicit and opt-in:

```bash
uv run python -m app.cli \
  --cases forgebench/cases/development \
  --provider forge \
  --provider-chain kimi-k3-deepseek \
  --model k3
```

The controller inherits `FORGE_CUSTOM_OPENAI_API_KEY` and `DEEPSEEK_API_KEY`
from the invoking process. Each child receives its own provider/model/base-URL
selector and isolated Forge home; the controller never accepts a key as a CLI
argument. The chain is unavailable to `baseline` and `holdout` commands.

The CCSwitch Kimi integration used during development is represented as:

```text
primary:  provider=custom_openai, model=k3,
          base=https://api.kimi.com/coding/v1
fallback: provider=deepseek, model=deepseek-v4-flash
```

The Kimi route is an explicit ordinary-development eval route with its own
typed execution identity; it is not a third H1/H2 DeepSeek arm and cannot alter
the locked comparison or holdout route set.

The source credential remains in CCSwitch; a local operator may resolve it into
an environment variable for the lifetime of a single runner process. Never
print that variable or place it in a committed file.
