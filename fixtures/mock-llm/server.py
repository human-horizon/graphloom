#!/usr/bin/env python3
"""Mock OpenAI-compatible endpoint for Graphloom E2E tests.

GET  /v1/models           -> one fake model
POST /v1/chat/completions -> canned DSL from responses/ (project vs function
                             dispatched by a marker in the system prompt)
"""
import json
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path

RESPONSES = Path(__file__).parent / "responses"
PORT = 8399


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def _json(self, obj, code=200):
        body = json.dumps(obj).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        if self.path == "/v1/models":
            self._json({"object": "list", "data": [{"id": "mock-model", "object": "model"}]})
        else:
            self._json({"error": "not found"}, 404)

    def do_POST(self):
        if self.path != "/v1/chat/completions":
            self._json({"error": "not found"}, 404)
            return
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        system = body["messages"][0]["content"]
        user = body["messages"][1]["content"]
        if "FUNCTION-level" in system:
            name = "function.json"
        elif "FILE-level" in system:
            name = "file.json"
        elif "scope tree" in system or "ENTITY-labels" in system:
            name = "labels.json"
        elif '"language":"typescript"' in user or '"language": "typescript"' in user:
            name = "project_ts.json"
        else:
            name = "project.json"
        content = (RESPONSES / name).read_text()
        self._json({
            "id": "chatcmpl-mock",
            "object": "chat.completion",
            "model": body.get("model", "mock-model"),
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": content},
                "finish_reason": "stop",
            }],
        })


if __name__ == "__main__":
    HTTPServer(("127.0.0.1", PORT), Handler).serve_forever()
