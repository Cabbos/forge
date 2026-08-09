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

The CCSwitch Kimi integration used during development is represented as:

```text
primary:  provider=custom_openai, model=k3,
          base=https://api.kimi.com/coding/v1
fallback: provider=deepseek, model=deepseek-v4-flash
```

The source credential remains in CCSwitch; a local operator may resolve it into
an environment variable for the lifetime of a single runner process. Never
print that variable or place it in a committed file.
