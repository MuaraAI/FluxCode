# FluxCode debug helper: a small debugger built on the standard library's bdb,
# so debugging Python needs nothing installed (no debugpy). FluxCode starts
#
#     python -u fluxdbg.py PORT SCRIPT [ARGS...]
#
# and this file connects back to 127.0.0.1:PORT, exchanging one JSON object
# per line. The program keeps its own stdin/stdout/stderr, so input() works.
#
# Kept light on purpose:
#  - with no breakpoints the trace function is removed and the program runs
#    at full speed; it is put back only when a breakpoint or pause needs it;
#  - frames of files without breakpoints are not line-traced (bdb's default);
#  - values are summarised with reprlib and children are listed on request.

import bdb
import json
import linecache
import os
import queue
import reprlib
import socket
import sys
import threading
import traceback

_HELPER = os.path.normcase(os.path.abspath(__file__))
_BDB = os.path.normcase(os.path.abspath(bdb.__file__))
_MAX_CHILDREN = 300

_repr = reprlib.Repr()
_repr.maxstring = 160
_repr.maxother = 160
_repr.maxlist = _repr.maxtuple = _repr.maxset = _repr.maxfrozenset = _repr.maxdeque = 12
_repr.maxdict = 8
_repr.maxlevel = 3


def summary(value):
    try:
        return _repr.repr(value)
    except Exception as e:  # a broken __repr__ must not break the debugger
        return f"<{type(value).__name__}: repr failed: {e}>"


def has_children(value):
    if isinstance(value, (str, bytes, bytearray, int, float, complex, bool, type(None))):
        return False
    if isinstance(value, (dict, list, tuple, set, frozenset)):
        return len(value) > 0
    if isinstance(value, type) or callable(value) and not hasattr(value, "__dict__"):
        return False
    try:
        return bool(vars(value))
    except TypeError:
        return False


def children(value):
    """(name, value) pairs shown when a variable is expanded."""
    if isinstance(value, dict):
        items = []
        for i, (k, v) in enumerate(value.items()):
            if i >= _MAX_CHILDREN:
                break
            items.append((summary(k), v))
        return items
    if isinstance(value, (list, tuple)):
        return [(str(i), v) for i, v in enumerate(value[:_MAX_CHILDREN])]
    if isinstance(value, (set, frozenset)):
        return [(str(i), v) for i, v in enumerate(list(value)[:_MAX_CHILDREN])]
    try:
        attrs = vars(value)
    except TypeError:
        return []
    return [(k, v) for k, v in list(attrs.items())[:_MAX_CHILDREN] if not (k.startswith("__") and k.endswith("__"))]


class Channel:
    def __init__(self, port):
        self.sock = socket.create_connection(("127.0.0.1", port))
        self.sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        self.lock = threading.Lock()

    def send(self, obj):
        data = (json.dumps(obj, ensure_ascii=False, default=str) + "\n").encode("utf-8")
        with self.lock:
            try:
                self.sock.sendall(data)
            except OSError:
                pass

    def lines(self):
        buf = b""
        while True:
            try:
                chunk = self.sock.recv(65536)
            except OSError:
                return
            if not chunk:
                return
            buf += chunk
            while b"\n" in buf:
                line, buf = buf.split(b"\n", 1)
                if line.strip():
                    yield json.loads(line.decode("utf-8"))


class Flux(bdb.Bdb):
    def __init__(self, chan, mainfile, my_code):
        super().__init__()
        self.chan = chan
        self.mainfile = self.canonic(mainfile)
        self.my_code = os.path.normcase(os.path.abspath(my_code)) if my_code else None
        self.main_ident = threading.get_ident()
        self.commands = queue.Queue()
        self.paused = False
        self.pause_requested = False
        self.stop_on_entry = False
        self.waiting_for_main = True
        self.botframe = None  # set by bdb once the program runs
        self.stack = []  # [(frame, lineno)] innermost first, while paused
        self.refs = {}
        # Without settrace_all_threads (Python < 3.12) the trace cannot be put
        # back from another thread, so it is kept installed instead.
        self.can_reinstall = hasattr(threading, "settrace_all_threads")

    # ---------------------------------------------------------- tracing

    def trace_dispatch(self, frame, event, arg):
        if threading.get_ident() != self.main_ident:
            return None  # only the main thread is debugged
        return super().trace_dispatch(frame, event, arg)

    def set_continue(self):
        if self.can_reinstall:
            super().set_continue()  # drops the trace entirely when there are no breakpoints
        else:
            self._set_stopinfo(self.botframe, None, -1)

    def install_trace(self):
        """Make sure the main thread is traced again (from the reader thread)."""
        if self.can_reinstall:
            threading.settrace_all_threads(self.trace_dispatch)
        frame = sys._current_frames().get(self.main_ident)
        while frame is not None and frame is not self.botframe:
            if frame.f_trace is None:
                frame.f_trace = self.trace_dispatch
            frame = frame.f_back

    def is_my_code(self, filename):
        f = self.canonic(filename)
        if f.startswith("<") or f in (_HELPER, _BDB) or "site-packages" in f:
            return False
        return self.my_code is None or f.startswith(self.my_code)

    def user_call(self, frame, argument_list):
        pass  # step into stops at the first line, not at the call

    def user_return(self, frame, return_value):
        pass  # step over/out stop at the next line of the caller

    def user_line(self, frame):
        filename = self.canonic(frame.f_code.co_filename)
        if self.waiting_for_main:
            if filename != self.mainfile or frame.f_lineno <= 0:
                return
            self.waiting_for_main = False
            if not self.stop_on_entry:
                self.set_continue()
                return
            return self.interaction(frame, "entry")
        if frame.f_lineno in self.breaks.get(filename, ()):
            reason = "breakpoint"
        elif self.pause_requested:
            reason = "pause"
        else:
            if not self.is_my_code(filename):
                return  # "just my code": keep stepping through libraries
            reason = "step"
        self.interaction(frame, reason)

    # ------------------------------------------------------ breakpoints

    def set_breakpoints(self, path, lines):
        shown = path
        path = self.canonic(path)
        self.clear_all_file_breaks(path)
        linecache.checkcache(path)
        placed = []
        for line in lines:
            line = self.code_line(path, int(line))
            if line and line not in placed and not self.set_break(path, line):
                placed.append(line)
        self.chan.send({"event": "breakpoints", "path": shown, "lines": placed})
        if placed and not self.paused and self.botframe is not None:
            self.install_trace()  # added while running: trace again

    @staticmethod
    def code_line(path, line):
        """The line a breakpoint really lands on: blank and comment lines move down."""
        for n in range(line, line + 30):
            text = linecache.getline(path, n)
            if not text:
                return None
            stripped = text.strip()
            if stripped and not stripped.startswith("#"):
                return n
        return None

    # ------------------------------------------------------------ pause

    def interaction(self, frame, reason, tb=None, text=None):
        if tb is not None:
            stack = []
            while tb is not None:
                stack.append((tb.tb_frame, tb.tb_lineno))
                tb = tb.tb_next
            stack.reverse()
        else:
            stack = []
            f = frame
            while f is not None and f is not self.botframe:
                stack.append((f, f.f_lineno))
                f = f.f_back
        self.stack = [(f, n) for f, n in stack if self.canonic(f.f_code.co_filename) not in (_HELPER, _BDB)]
        self.refs = {}
        self.pause_requested = False
        frames = [
            {"id": i, "name": f.f_code.co_name, "path": os.path.abspath(f.f_code.co_filename), "line": n,
             "internal": not self.is_my_code(f.f_code.co_filename)}
            for i, (f, n) in enumerate(self.stack)
        ]
        self.paused = True
        self.chan.send({"event": "stopped", "reason": reason, "text": text, "frames": frames})
        top = self.stack[0][0] if self.stack else frame
        while True:
            cmd = self.commands.get()
            op = cmd.get("op")
            if op == "continue":
                self.set_continue()
            elif op == "next":
                self.set_next(top)
            elif op == "stepIn":
                self.set_step()
            elif op == "stepOut":
                self.set_return(top)
            else:
                self.reply(cmd)
                continue
            break
        self.paused = False
        self.stack = []
        self.refs = {}
        self.chan.send({"event": "continued"})

    # -------------------------------------------------------- inspection

    def ref(self, value):
        if not has_children(value):
            return 0
        n = len(self.refs) + 1
        self.refs[n] = value
        return n

    def describe(self, name, value):
        return {"name": name, "value": summary(value), "type": type(value).__name__, "ref": self.ref(value)}

    def scope_items(self, frame_id):
        frame = self.stack[frame_id][0]
        if frame.f_locals is frame.f_globals:
            local = {k: v for k, v in frame.f_globals.items()
                     if not (k.startswith("__") and k.endswith("__")) and type(v).__name__ != "module"}
            return [("Locals", local)]
        glob = {k: v for k, v in frame.f_globals.items()
                if not (k.startswith("__") and k.endswith("__")) and type(v).__name__ != "module"}
        return [("Locals", dict(frame.f_locals)), ("Globals", glob)]

    def reply(self, cmd):
        seq = cmd.get("seq")
        try:
            body = self.answer(cmd)
            self.chan.send({"event": "response", "seq": seq, "body": body})
        except Exception as e:
            self.chan.send({"event": "response", "seq": seq, "error": f"{type(e).__name__}: {e}"})

    def answer(self, cmd):
        op = cmd.get("op")
        if op == "scopes":
            out = []
            for name, items in self.scope_items(int(cmd.get("frame", 0))):
                n = len(self.refs) + 1
                self.refs[n] = ("scope", items)
                out.append({"name": name, "ref": n, "expensive": name == "Globals"})
            return out
        if op == "variables":
            target = self.refs.get(int(cmd["ref"]))
            if target is None:
                return []
            if isinstance(target, tuple) and len(target) == 2 and target[0] == "scope":
                pairs = list(target[1].items())[:_MAX_CHILDREN]
            else:
                pairs = children(target)
            return [self.describe(k, v) for k, v in pairs]
        if op == "evaluate":
            frame = self.stack[int(cmd.get("frame", 0))][0] if self.stack else None
            if frame is None:
                raise RuntimeError("Not paused")
            expr = cmd["expression"]
            try:
                code = compile(expr, "<watch>", "eval")
            except SyntaxError:
                exec(compile(expr, "<console>", "exec"), frame.f_globals, frame.f_locals)
                return {"value": "", "type": "", "ref": 0}
            value = eval(code, frame.f_globals, frame.f_locals)
            return self.describe(expr, value)
        raise RuntimeError(f"Unknown request {op!r}")


def reader(dbg):
    """Socket messages: breakpoints and pause apply at once, the rest waits for a pause."""
    for msg in dbg.chan.lines():
        op = msg.get("op")
        if op == "setBreakpoints":
            dbg.set_breakpoints(msg["path"], msg.get("lines", []))
        elif op == "pause":
            if not dbg.paused:
                dbg.pause_requested = True
                dbg.set_step()
                dbg.install_trace()
        elif op == "init" or dbg.paused:
            dbg.commands.put(msg)
        elif msg.get("seq") is not None:
            dbg.chan.send({"event": "response", "seq": msg["seq"], "error": "The program is running"})
    os._exit(0)  # FluxCode went away: end the program too


def user_frames(tb):
    while tb is not None and os.path.normcase(os.path.abspath(tb.tb_frame.f_code.co_filename)) in (_HELPER, _BDB):
        tb = tb.tb_next
    return tb


def main():
    port = int(sys.argv[1])
    script = os.path.abspath(sys.argv[2])
    sys.argv = [script] + sys.argv[3:]
    sys.path[0] = os.path.dirname(script)

    chan = Channel(port)
    chan.send({"event": "ready", "python": sys.version.split()[0]})
    dbg = Flux(chan, script, None)
    threading.Thread(target=reader, args=(dbg,), daemon=True, name="fluxdbg").start()
    init = dbg.commands.get()
    dbg.my_code = os.path.normcase(os.path.abspath(init["root"])) if init.get("root") else None
    dbg.stop_on_entry = bool(init.get("stopOnEntry"))
    for path, lines in (init.get("breakpoints") or {}).items():
        dbg.set_breakpoints(path, lines)

    import __main__
    __main__.__dict__.clear()
    __main__.__dict__.update({"__name__": "__main__", "__file__": script, "__builtins__": __builtins__})
    status = 0
    try:
        with open(script, "rb") as f:
            code = compile(f.read(), script, "exec")
        dbg.run(code, __main__.__dict__)
    except SystemExit as e:
        status = e.code if isinstance(e.code, int) else (0 if e.code is None else 1)
        if e.code is not None and not isinstance(e.code, int):
            print(e.code, file=sys.stderr)
    except bdb.BdbQuit:
        pass
    except BaseException as e:
        sys.settrace(None)
        tb = user_frames(e.__traceback__)
        traceback.print_exception(type(e), e, tb)
        sys.stderr.flush()
        if tb is not None:
            dbg.interaction(None, "exception", tb=tb, text=f"{type(e).__name__}: {e}")
        status = 1
    sys.exit(status)


if __name__ == "__main__":
    # Re-import under its own name so clearing __main__ (for the program) keeps ours.
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import fluxdbg
    sys.path.pop(0)
    fluxdbg.main()
