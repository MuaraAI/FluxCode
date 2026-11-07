// Debugger demo: click left of line 6 to set a breakpoint, then press F5.
// F10 steps over, F11 steps into, F5 continues. Hover `total` while paused.

function area(radius) {
  const squared = radius * radius;
  return Math.PI * squared;
}

const shapes = [{ name: "small", r: 1 }, { name: "medium", r: 2 }, { name: "large", r: 3 }];
let total = 0;
for (const shape of shapes) {
  total += area(shape.r);
}

console.log(`${shapes.length} circles, total area ${total.toFixed(2)}`);
