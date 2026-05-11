#!/usr/bin/env python3
"""LSP smoke test for frac_lang_lsp.

Spawns the LSP binary, opens a .frac file, and prints hover responses at
a few hand-picked positions.  Useful for verifying the server end-to-end
without going through Cursor/VS Code.

Usage:
    python3 smoke_test.py [<path-to-frac-file>]

Defaults to patterns/wa.frac relative to the repo root.  Builds against
target/debug/frac_lang_lsp.
"""
import json, os, subprocess, sys, time

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
REPO_ROOT = os.path.abspath(os.path.join(SCRIPT_DIR, "..", ".."))
BIN = os.path.join(REPO_ROOT, "target", "debug", "frac_lang_lsp")

SRC_PATH = sys.argv[1] if len(sys.argv) > 1 else os.path.join(REPO_ROOT, "patterns", "wa.frac")
URI = "file://" + SRC_PATH

with open(SRC_PATH) as f:
    src = f.read()

proc = subprocess.Popen([BIN], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)

def send(payload):
    body = json.dumps(payload).encode()
    proc.stdin.write(f"Content-Length: {len(body)}\r\n\r\n".encode() + body)
    proc.stdin.flush()

def read_message():
    n = 0
    line = proc.stdout.readline()
    while line and line != b"\r\n":
        if line.startswith(b"Content-Length:"):
            n = int(line.split(b":")[1].strip())
        line = proc.stdout.readline()
    body = proc.stdout.read(n)
    return json.loads(body)

def read_response_for(id):
    """Skip notifications; find the response with the given id."""
    while True:
        msg = read_message()
        if msg.get("id") == id:
            return msg
        if msg.get("method") == "textDocument/publishDiagnostics":
            diags = msg["params"]["diagnostics"]
            if diags:
                print(f"[diagnostics] {len(diags)} issue(s):")
                for d in diags:
                    r = d["range"]["start"]
                    print(f"  - L{r['line']+1}:{r['character']+1}  {d['message']}")
            else:
                print("[diagnostics] clean")

def hover_at(id, line, character, label):
    send({"jsonrpc":"2.0","id":id,"method":"textDocument/hover",
          "params":{"textDocument":{"uri":URI},"position":{"line":line,"character":character}}})
    resp = read_response_for(id)
    print(f"=== HOVER: {label} (line {line+1}, col {character+1}) ===")
    if resp.get("result"):
        print(resp["result"]["contents"]["value"])
    else:
        print("(no hover info)")
    print()

send({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{},"processId":None,"rootUri":None}})
read_response_for(1)

send({"jsonrpc":"2.0","method":"initialized","params":{}})
send({"jsonrpc":"2.0","method":"textDocument/didOpen",
      "params":{"textDocument":{"uri":URI,"languageId":"frac","version":1,"text":src}}})

time.sleep(0.3)

# Targets tuned for wa.frac.  Override by passing different positions.
hover_at(2,  5,  6, "tile tri")
hover_at(3, 22, 17, "partition tri.tri_split")
hover_at(4, 49, 10, "child 0 in quad.quad_split")

send({"jsonrpc":"2.0","id":99,"method":"shutdown"})
read_response_for(99)
send({"jsonrpc":"2.0","method":"exit"})
try:
    proc.wait(timeout=2)
except subprocess.TimeoutExpired:
    proc.kill()
