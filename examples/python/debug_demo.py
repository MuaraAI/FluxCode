"""Debugger demo: click left of line 12 to set a breakpoint, then press F5.

F10 steps over, F11 steps into area(), Shift+F11 steps out, F5 continues.
Hover `total` while paused, or type `shapes[0]` in the Output input row.
"""

import math


def area(radius):
    squared = radius * radius
    return math.pi * squared


shapes = [{"name": "small", "r": 1}, {"name": "medium", "r": 2}, {"name": "large", "r": 3}]
total = 0.0
for shape in shapes:
    total += area(shape["r"])

print(f"{len(shapes)} circles, total area {total:.2f}")
