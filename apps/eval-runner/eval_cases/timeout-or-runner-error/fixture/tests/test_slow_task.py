from src.slow_task import run


def test_run_returns_value() -> None:
    assert run() == "done"
