"""Tests for calc module."""

from calc import add, subtract, multiply, divide, power, factorial
import pytest


class TestCalc:
    def test_add(self):
        assert add(2, 3) == 5
        assert add(-1, 1) == 0

    def test_subtract(self):
        assert subtract(5, 3) == 2
        assert subtract(0, 5) == -5

    def test_multiply(self):
        assert multiply(3, 4) == 12
        assert multiply(0, 5) == 0

    def test_divide(self):
        assert divide(10, 2) == 5.0
        assert divide(7, 2) == 3.5

    def test_divide_by_zero(self):
        with pytest.raises(ValueError):
            divide(1, 0)

    def test_power(self):
        """This test will FAIL until power() is fixed."""
        assert power(2, 3) == 8   # 2^3 = 8
        assert power(5, 0) == 1   # any^0 = 1

    def test_factorial(self):
        """This test will FAIL until factorial() is fixed (stack overflow)."""
        assert factorial(0) == 1
        assert factorial(5) == 120
