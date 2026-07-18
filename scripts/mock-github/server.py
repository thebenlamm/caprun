#!/usr/bin/env python3
"""Stdlib-only mock GitHub HTTPS endpoint for the Phase-40 composed live proof.

Stands in for GitHub's write API so a REAL `github.pr` POST completes over REAL
TLS while riding the SHIPPED broker egress path (validate_url -> allowlist ->
resolve-and-pin) unchanged. It answers HTTP 201 with a plausible created-PR JSON
to `POST /repos/<owner>/<repo>/pulls`, and 404 to everything else.

NO third-party dependency: only `http.server` + `ssl` from the standard library
(honours CLAUDE.md "no new package-manager dependency"; runs on `python:3-slim`).

TLS: presents `certs/github-mock.caprun.test.{pem,key}` — the SAME self-signed
cert the broker trusts under the non-default `mock-egress-ca` cargo feature
(`crates/brokerd/tests/fixtures/mock-egress-ca.der` is its DER encoding). The
domain `github-mock.caprun.test` is a reserved non-resolvable TLD (RFC 6761):
test-only, no production trust.

Usage (inside the compose-verify sidecar): `python3 server.py` — binds 0.0.0.0:443.
Override the port with MOCK_GITHUB_PORT (default 443).
"""
import json
import os
import ssl
import sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

CERT_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "certs")
CERT_FILE = os.path.join(CERT_DIR, "github-mock.caprun.test.pem")
KEY_FILE = os.path.join(CERT_DIR, "github-mock.caprun.test.key")
PORT = int(os.environ.get("MOCK_GITHUB_PORT", "443"))


def _is_pulls_path(path: str) -> bool:
    """True for `/repos/<owner>/<repo>/pulls` (ignoring any query string)."""
    path = path.split("?", 1)[0]
    parts = [p for p in path.split("/") if p]
    return len(parts) == 4 and parts[0] == "repos" and parts[3] == "pulls"


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def _send(self, status: int, payload: dict) -> None:
        body = json.dumps(payload).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_POST(self) -> None:  # noqa: N802 (http.server naming)
        length = int(self.headers.get("Content-Length", "0") or "0")
        # Drain the request body so the connection can be reused / closed cleanly.
        if length:
            self.rfile.read(length)
        if _is_pulls_path(self.path):
            # A plausible created-PR response: enough for the opaque success
            # event + CAS to be exercised. No real GitHub data.
            self._send(201, {
                "number": 1,
                "state": "open",
                "html_url": "https://github-mock.caprun.test/mock/mock/pull/1",
                "id": 1,
                "title": "mock",
            })
        else:
            self._send(404, {"message": "Not Found (mock github: only POST /repos/*/pulls)"})

    def do_GET(self) -> None:  # noqa: N802
        self._send(404, {"message": "Not Found (mock github: only POST /repos/*/pulls)"})

    def log_message(self, fmt, *args):  # keep the sidecar log readable
        sys.stderr.write("mock-github: " + (fmt % args) + "\n")


def main() -> None:
    ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    ctx.load_cert_chain(certfile=CERT_FILE, keyfile=KEY_FILE)
    httpd = ThreadingHTTPServer(("0.0.0.0", PORT), Handler)
    httpd.socket = ctx.wrap_socket(httpd.socket, server_side=True)
    sys.stderr.write(f"mock-github: HTTPS listening on 0.0.0.0:{PORT} "
                     f"(cert CN=github-mock.caprun.test)\n")
    httpd.serve_forever()


if __name__ == "__main__":
    main()
