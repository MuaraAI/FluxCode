// Flux Tasks: tiny to-do app (plain JavaScript, saved in localStorage).
const KEY = "flux-tasks";
const $ = (s) => document.querySelector(s);

let tasks = JSON.parse(localStorage.getItem(KEY) ?? "null") ?? [
  { title: "Buka examples/web di FluxCode", done: true },
  { title: "Tekan F5 untuk Live Preview", done: false },
  { title: "Ubah warna di style.css", done: false },
];

function save() {
  localStorage.setItem(KEY, JSON.stringify(tasks));
}

function render() {
  const list = $("#list");
  list.replaceChildren(
    ...tasks.map((task, i) => {
      const li = document.createElement("li");
      li.className = task.done ? "done" : "";
      const box = document.createElement("input");
      box.type = "checkbox";
      box.checked = task.done;
      box.addEventListener("change", () => {
        tasks[i].done = box.checked;
        save();
        render();
      });
      const span = document.createElement("span");
      span.textContent = task.title;
      li.append(box, span);
      return li;
    }),
  );
  const open = tasks.filter((t) => !t.done).length;
  $("#count").textContent = `${open} tugas tersisa dari ${tasks.length}`;
}

$("#add").addEventListener("submit", (e) => {
  e.preventDefault();
  const title = $("#title").value.trim();
  if (!title) return;
  tasks.push({ title, done: false });
  $("#title").value = "";
  save();
  render();
});

$("#clear").addEventListener("click", () => {
  tasks = tasks.filter((t) => !t.done);
  save();
  render();
});

render();
