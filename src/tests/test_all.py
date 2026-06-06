import pytest
import risio


def test_sum_as_string():
    assert risio.sum_as_string(1, 1) == "2"
