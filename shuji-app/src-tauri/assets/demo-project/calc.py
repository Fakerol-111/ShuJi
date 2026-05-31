"""Simple calculator module — demo project for ShuJi."""


def add(a: int, b: int) -> int:
    return a + b


def subtract(a: int, b: int) -> int:
    return a - b


def multiply(a: int, b: int) -> int:
    return a * b


def divide(a: int, b: int) -> float:
    if b == 0:
        raise ValueError("Cannot divide by zero")
    return a / b


# Known issues (for the emperor to fix)

def power(base: int, exp: int) -> int:
    """Bug: should return base ** exp, but returns base * exp instead."""
    return base * exp  # TODO: fix me


def factorial(n: int) -> int:
    """Bug: missing base case, infinite recursion for n <= 0."""
    return n * factorial(n - 1)  # TODO: fix me
