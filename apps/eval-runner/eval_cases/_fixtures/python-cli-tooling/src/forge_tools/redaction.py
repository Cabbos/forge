import re


SECRET_PATTERNS = [
    re.compile(r"(FORGE_TOKEN=)[^\\s]+"),
    re.compile(r"(OPENAI_API_KEY=)[^\\s]+"),
]


def redact_env_output(text: str) -> str:
    result = text
    for pattern in SECRET_PATTERNS:
        result = pattern.sub(r"\\1[REDACTED]", result)
    return result
