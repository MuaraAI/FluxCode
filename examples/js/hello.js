// FluxCode JavaScript smoke test: run with F5 (Node.js).
import { createInterface } from "node:readline/promises";

const items = [
  { name: "kopi", price: 18_000 },
  { name: "roti", price: 12_500 },
];

console.log(`Node ${process.version} running in FluxCode ⚡`);
for (const { name, price } of items) {
  console.log(`  ${name.padEnd(6)} x3 = Rp${(price * 3).toLocaleString("id-ID")}`);
}

const fib = (n) => {
  let [a, b] = [0n, 1n];
  for (let i = 0; i < n; i++) [a, b] = [b, a + b];
  return a;
};
const t0 = performance.now();
console.log("fib(90) =", fib(90).toString());
console.log(`computed in ${(performance.now() - t0).toFixed(3)} ms`);

const rl = createInterface({ input: process.stdin, output: process.stdout });
const name = await rl.question("Siapa namamu? ");
console.log(`Halo, ${name}! 👋`);
rl.close();
