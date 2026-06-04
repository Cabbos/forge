from src.calculator import add_one


def test_add_one_returns_next_integer() -> None:
    assert add_one(2) == 3
