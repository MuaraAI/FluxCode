// Python: standard-library and popular PyPI modules after `import` / `from`,
// workspace requirements first, dunder methods and snippets. Keywords,
// builtins and local names come from @codemirror/lang-python.

import { snippetCompletion as snip } from "@codemirror/autocomplete";
import { workspace } from "./workspace.js";

const words = (s) => s.trim().split(/\s+/);

export const STDLIB = words(`
  os os.path sys re json math random time datetime collections itertools functools pathlib typing dataclasses enum abc logging argparse subprocess shutil glob
  csv sqlite3 urllib urllib.request urllib.parse http http.server http.client socket threading multiprocessing asyncio concurrent.futures queue
  unittest unittest.mock doctest pprint copy io string textwrap struct hashlib hmac secrets base64 uuid decimal fractions statistics operator contextlib
  tempfile zipfile tarfile gzip pickle shelve configparser tomllib xml.etree.ElementTree html html.parser email smtplib imaplib ftplib ssl
  tkinter tkinter.ttk tkinter.messagebox tkinter.filedialog turtle curses getpass platform signal traceback warnings inspect dis gc weakref
  heapq bisect array types numbers cmath zoneinfo calendar locale gettext codecs unicodedata difflib fnmatch stat filecmp mimetypes webbrowser
  timeit cProfile profile pdb venv importlib importlib.resources pkgutil zipimport sched selectors mmap ctypes wsgiref xmlrpc ipaddress graphlib`);

export const PYPI_POPULAR = words(`
  numpy pandas requests flask django fastapi uvicorn starlette sqlalchemy pydantic matplotlib seaborn scipy sklearn torch torchvision tensorflow keras transformers
  openai anthropic langchain httpx aiohttp bs4 lxml selenium playwright pytest black ruff mypy PIL cv2 pygame PyQt5 PyQt6 PySide6 kivy customtkinter
  streamlit gradio dash plotly bokeh jupyter IPython click typer rich tqdm colorama dotenv yaml toml jinja2 markdown celery redis pymongo psycopg2 mysql.connector
  boto3 schedule apscheduler paramiko fabric win32api win32gui pyautogui pynput keyboard mouse discord telebot telegram tweepy yt_dlp moviepy pydub
  speech_recognition pyttsx3 gtts openpyxl xlrd docx reportlab PyPDF2 pypdf pdfplumber tabulate loguru attrs marshmallow alembic peewee tortoise motor
  websockets grpc cryptography jwt passlib bcrypt nltk spacy gensim scrapy pyperclip pyinstaller setuptools wheel pip virtualenv poetry
  networkx sympy statsmodels xgboost lightgbm catboost optuna mlflow wandb datasets accelerate diffusers sentence_transformers faiss chromadb
  pillow imageio skimage pytesseract qrcode barcode pyzbar sounddevice pyaudio soundfile librosa numba cython dask polars pyarrow duckdb
  flask_sqlalchemy flask_cors flask_login flask_wtf wtforms djangorestframework rest_framework channels gunicorn waitress tornado sanic bottle
  pytest_asyncio hypothesis faker factory_boy freezegun responses vcr coverage tox pre_commit isort flake8 pylint bandit`);

const DUNDER = words(`__init__ __str__ __repr__ __len__ __getitem__ __setitem__ __delitem__ __contains__ __iter__ __next__ __eq__ __lt__ __le__ __gt__ __ge__ __hash__
  __call__ __enter__ __exit__ __add__ __sub__ __mul__ __truediv__ __bool__ __getattr__ __setattr__ __new__ __del__ __name__ __file__ __dict__ __class__ __doc__ __slots__ __all__ __version__`)
  .map((d) => ({ label: d, type: "method", detail: "dunder" }));

const SNIPPETS = [
  snip("if __name__ == \"__main__\":\n\t${main()}", { label: "ifmain", detail: "main guard", type: "snippet" }),
  snip("def ${name}(${params}):\n\t${pass}", { label: "def", detail: "function", type: "snippet" }),
  snip("def ${name}(self${, params}):\n\t${pass}", { label: "defs", detail: "method", type: "snippet" }),
  snip("async def ${name}(${params}):\n\t${pass}", { label: "adef", detail: "async function", type: "snippet" }),
  snip("def __init__(self${, params}):\n\t${}", { label: "init", detail: "__init__", type: "snippet" }),
  snip("class ${Name}:\n\tdef __init__(self${, params}):\n\t\t${}", { label: "class", detail: "class with __init__", type: "snippet" }),
  snip("@dataclass\nclass ${Name}:\n\t${field}: ${type}", { label: "dataclass", detail: "from dataclasses import dataclass", type: "snippet" }),
  snip("for ${item} in ${items}:\n\t${}", { label: "for", detail: "for loop", type: "snippet" }),
  snip("for ${i}, ${item} in enumerate(${items}):\n\t${}", { label: "fore", detail: "for with enumerate", type: "snippet" }),
  snip("for ${i} in range(${n}):\n\t${}", { label: "forr", detail: "for in range", type: "snippet" }),
  snip("while ${cond}:\n\t${}", { label: "while", detail: "while loop", type: "snippet" }),
  snip("if ${cond}:\n\t${}\nelse:\n\t${}", { label: "ifelse", detail: "if / else", type: "snippet" }),
  snip("try:\n\t${}\nexcept ${Exception} as e:\n\t${print(e)}", { label: "try", detail: "try / except", type: "snippet" }),
  snip("try:\n\t${}\nexcept ${Exception} as e:\n\t${print(e)}\nfinally:\n\t${}", { label: "tryf", detail: "try / except / finally", type: "snippet" }),
  snip("with open(${\"file.txt\"}, \"${r}\", encoding=\"utf-8\") as f:\n\t${}", { label: "with", detail: "with open(...)", type: "snippet" }),
  snip("lambda ${x}: ${}", { label: "lambda", detail: "lambda", type: "snippet" }),
  snip("[${x} for ${x} in ${items}]", { label: "lc", detail: "list comprehension", type: "snippet" }),
  snip("{${k}: ${v} for ${k}, ${v} in ${items}.items()}", { label: "dc", detail: "dict comprehension", type: "snippet" }),
  snip("print(f\"${}\")", { label: "printf", detail: "print f-string", type: "snippet" }),
  snip("@property\ndef ${name}(self):\n\treturn self._${name}", { label: "property", detail: "property getter", type: "snippet" }),
  snip("@staticmethod\ndef ${name}(${params}):\n\t${pass}", { label: "staticmethod", detail: "static method", type: "snippet" }),
  snip("@classmethod\ndef ${name}(cls${, params}):\n\t${pass}", { label: "classmethod", detail: "class method", type: "snippet" }),
  snip("import argparse\n\nparser = argparse.ArgumentParser(description=\"${}\")\nparser.add_argument(\"${name}\")\nargs = parser.parse_args()", { label: "argparse", detail: "argument parser", type: "snippet" }),
  snip("import logging\n\nlogging.basicConfig(level=logging.${INFO})\nlogger = logging.getLogger(__name__)", { label: "logging", detail: "logger setup", type: "snippet" }),
  snip("from flask import Flask, jsonify, request\n\napp = Flask(__name__)\n\n\n@app.route(\"/\")\ndef index():\n\treturn ${\"Hello\"}\n\n\nif __name__ == \"__main__\":\n\tapp.run(debug=True)", { label: "flask", detail: "Flask app", type: "snippet" }),
  snip("from fastapi import FastAPI\n\napp = FastAPI()\n\n\n@app.get(\"/\")\nasync def root():\n\treturn {\"message\": ${\"Hello\"}}", { label: "fastapi", detail: "FastAPI app", type: "snippet" }),
  snip("import tkinter as tk\n\nroot = tk.Tk()\nroot.title(\"${App}\")\n${}\nroot.mainloop()", { label: "tkinter", detail: "Tk window", type: "snippet" }),
  snip("\"\"\"${description}\"\"\"", { label: "docstring", detail: "docstring", type: "snippet" }),
  snip("async def main():\n\t${}\n\nasyncio.run(main())", { label: "asyncmain", detail: "asyncio main", type: "snippet" }),
];

const moduleOptions = () => [
  ...workspace.pypi.map((m) => ({ label: m.replace(/-/g, "_").toLowerCase(), type: "module", detail: "requirements", boost: 20 })),
  ...STDLIB.map((m) => ({ label: m, type: "module", detail: "stdlib", boost: 10 })),
  ...PYPI_POPULAR.map((m) => ({ label: m, type: "module", detail: "PyPI" })),
];

function importCompletion(context) {
  const m = context.matchBefore(/^\s*(?:import|from)\s+[\w.]*$/) ?? context.matchBefore(/^\s*import\s+[\w.]+\s*,\s*[\w.]*$/);
  if (!m) return null;
  const word = context.matchBefore(/[\w.]*$/);
  return { from: word.from, options: moduleOptions(), validFor: /^[\w.]*$/ };
}

export function pythonCompletion() {
  return [
    importCompletion,
    (context) => {
      const word = context.matchBefore(/[\w]*$/);
      if (!word || (!word.text && !context.explicit)) return null;
      const afterDot = context.matchBefore(/\.\w*$/);
      return { from: word.from, options: afterDot ? DUNDER : [...SNIPPETS, ...DUNDER], validFor: /^\w*$/ };
    },
  ];
}
