"""A small desktop app (tkinter): run it with F5 to open a window."""

import tkinter as tk
from tkinter import ttk


class Counter(tk.Tk):
    def __init__(self) -> None:
        super().__init__()
        self.title("Flux Counter")
        self.geometry("320x180")
        self.configure(bg="#181b22")
        self.count = 0

        style = ttk.Style(self)
        style.theme_use("clam")
        style.configure("TButton", padding=8, font=("Segoe UI", 11))

        self.label = tk.Label(self, text="0", font=("Segoe UI", 40, "bold"), fg="#e6e9ef", bg="#181b22")
        self.label.pack(pady=(18, 8))

        row = tk.Frame(self, bg="#181b22")
        row.pack()
        ttk.Button(row, text="−", command=lambda: self.bump(-1)).pack(side="left", padx=6)
        ttk.Button(row, text="+", command=lambda: self.bump(1)).pack(side="left", padx=6)
        print("Flux Counter window opened")

    def bump(self, step: int) -> None:
        self.count += step
        self.label.config(text=str(self.count))
        print(f"count = {self.count}")


if __name__ == "__main__":
    Counter().mainloop()
