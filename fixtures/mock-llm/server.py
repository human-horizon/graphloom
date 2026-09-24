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
        if "# Scope tree items" in system or "ENTITY-labels" in system:
            content = strict_labels_response(system)
        elif "FUNCTION-level" in system:
            content = (RESPONSES / "function.json").read_text()
        elif "FILE-level" in system:
            content = (RESPONSES / "file.json").read_text()
        elif '"language":"typescript"' in user or '"language": "typescript"' in user:
            content = (RESPONSES / "project_ts.json").read_text()
        else:
            content = (RESPONSES / "project.json").read_text()
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


def strict_labels_response(system):
    known = json.loads((RESPONSES / "labels.json").read_text()).get("labels", {})
    marker = "Current IDs that must be described now:\n"
    if marker not in system:
        return json.dumps({"labels": known})
    ids_text = system.split(marker, 1)[1].split("\n\n# Scope tree items", 1)[0]
    labels = {}
    for entity_id in (line.strip() for line in ids_text.splitlines()):
        if not entity_id:
            continue
        entry = known.get(entity_id, {})
        labels[entity_id] = {
            "label": entry.get("label") or "Выполнить операцию",
            "summary": entry.get("summary") or "Этот блок выполняет действие, описанное исходным кодом."
        }
    return json.dumps({"labels": labels}, ensure_ascii=False)


if __name__ == "__main__":
    HTTPServer(("127.0.0.1", PORT), Handler).serve_forever()
