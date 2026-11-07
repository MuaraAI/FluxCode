// TypeScript runs directly on Node 23.6+ (types are stripped), F5 in FluxCode.
interface User {
  name: string;
  langs: string[];
}

const users: User[] = [
  { name: "Ayu", langs: ["Python", "JavaScript"] },
  { name: "Bima", langs: ["HTML", "CSS", "TypeScript"] },
];

function describe(user: User): string {
  return `${user.name} suka ${user.langs.join(", ")}`;
}

for (const u of users) console.log(describe(u));
console.log(`total bahasa: ${new Set(users.flatMap((u) => u.langs)).size}`);
