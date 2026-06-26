"""Minimal OpenAI-compatible /embeddings endpoint serving local all-MiniLM-L6-v2
(384-dim) — so SAG (and anything else) can use the SAME embedding as mem1/mem0
for an apples-to-apples comparison. No API key needed.

Run: python3.11 /tmp/local_embed_server.py  (listens on :8090)
POST /v1/embeddings  {"model":"...", "input": "text" | ["t1","t2"]}
-> {"data":[{"embedding":[...384...],"index":0}], "model":..., "usage":{...}}
"""
import warnings, json
warnings.filterwarnings("ignore")
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from sentence_transformers import SentenceTransformer

MODEL = SentenceTransformer("sentence-transformers/all-MiniLM-L6-v2")
print("local-embed: all-MiniLM-L6-v2 loaded (dim=384), listening on :8090", flush=True)


class H(BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def do_POST(self):
        try:
            n = int(self.headers.get("Content-Length", 0))
            body = json.loads(self.rfile.read(n) or b"{}")
            inp = body.get("input", [])
            texts = [inp] if isinstance(inp, str) else list(inp)
            vecs = MODEL.encode(texts, normalize_embeddings=False).tolist()
            out = {
                "object": "list",
                "model": body.get("model", "all-MiniLM-L6-v2"),
                "data": [{"object": "embedding", "index": i, "embedding": v} for i, v in enumerate(vecs)],
                "usage": {"prompt_tokens": 0, "total_tokens": 0},
            }
            payload = json.dumps(out).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)
        except Exception as e:
            msg = json.dumps({"error": {"message": str(e)}}).encode()
            self.send_response(500)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(msg)))
            self.end_headers()
            self.wfile.write(msg)


if __name__ == "__main__":
    ThreadingHTTPServer(("127.0.0.1", 8090), H).serve_forever()
