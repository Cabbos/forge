from src.settings import default_region


def test_default_region_is_stable() -> None:
    assert default_region() == "us-east-1"
