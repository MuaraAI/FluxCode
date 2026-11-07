"""FluxCode Python smoke test: output, input(), unicode and timing."""

import sys
import time
from dataclasses import dataclass


@dataclass
class Item:
    name: str
    price: float

    def total(self, qty: int) -> float:
        return self.price * qty


def fib(n: int) -> int:
    a, b = 0, 1
    for _ in range(n):
        a, b = b, a + b
    return a


def main() -> None:
    print(f"Python {sys.version.split()[0]} running in FluxCode ⚡")
    items = [Item("kopi", 18_000), Item("roti", 12_500)]
    for item in items:
        print(f"  {item.name:<6} x3 = Rp{item.total(3):,.0f}")

    start = time.perf_counter()
    print("fib(90) =", fib(90))
    print(f"computed in {(time.perf_counter() - start) * 1000:.3f} ms")

    name = input("Siapa namamu? ")
    print(f"Halo, {name}! 👋")


if __name__ == "__main__":
    main()
