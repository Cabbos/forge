from src.workflow import status_label


def test_status_label_distinguishes_completed_work() -> None:
    assert status_label(True) == "done"
    assert status_label(False) == "pending"
