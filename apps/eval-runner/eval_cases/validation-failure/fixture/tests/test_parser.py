from src.parser import parse_int


def test_parse_int_rejects_empty_text() -> None:
    try:
        parse_int("")
    except ValueError:
        return
    raise AssertionError("empty text must raise ValueError")
