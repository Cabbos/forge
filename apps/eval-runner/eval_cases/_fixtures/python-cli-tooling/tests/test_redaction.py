from forge_tools.redaction import redact_env_output


def test_redacts_forge_token() -> None:
    assert redact_env_output("FORGE_TOKEN=secret-123") == "FORGE_TOKEN=[REDACTED]"


def test_redacts_openai_api_key() -> None:
    assert redact_env_output("OPENAI_API_KEY=sk-live") == "OPENAI_API_KEY=[REDACTED]"


def test_keeps_non_secret_output() -> None:
    assert redact_env_output("PATH=/usr/bin") == "PATH=/usr/bin"
