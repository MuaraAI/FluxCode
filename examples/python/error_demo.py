"""Raises an error on purpose: the traceback lines in the Output panel are clickable."""


def divide(a: float, b: float) -> float:
    return a / b


def main() -> None:
    print("About to divide by zero...")
    divide(1, 0)


main()
