def retry_delay_seconds(attempt: int, *, base: float = 0.5, cap: float = 30.0) -> float:
    if attempt < 1:
        raise ValueError("attempt must start at 1")
    return min(cap, base * (2 ** (attempt - 1)))
