from event_worker.retry import retry_delay_seconds


def test_retry_backoff_starts_at_base_and_caps() -> None:
    assert [retry_delay_seconds(attempt, base=1, cap=5) for attempt in range(1, 6)] == [
        1,
        2,
        4,
        5,
        5,
    ]
