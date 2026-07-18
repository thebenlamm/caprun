#!/usr/bin/env python3
"""Stdlib-only mock GitHub HTTPS endpoint for the Phase-40 composed live proof.

Stands in for GitHub's write API so a REAL `github.pr` POST completes over REAL
TLS while riding the SHIPPED broker egress path (validate_url -> allowlist ->
resolve-and-pin) unchanged. It answers HTTP 201 with a plausible created-PR JSON
to `POST /repos/<owner>/<repo>/pulls`, and 404 to everything else.

## v1.9 Phase 44 (WG-9): git smart-HTTP `git-receive-pack` mock

Additionally stands in for a git remote's receive-pack endpoint so a REAL broker
`git.push` completes over REAL TLS while riding the SHIPPED frozen-IP two-request
transfer path (info/refs advertisement GET -> command-list+PACK POST -> report-
status parse) unchanged:

  * `GET  /<repo>/info/refs?service=git-receive-pack` -> a valid pkt-line ref
    advertisement (the `# service=git-receive-pack` announcement pkt + flush,
    then the empty-repo capabilities line, then a flush). The empty-repo
    advertisement signals a CREATE to the broker (the target ref is not
    advertised -> zero old-oid), which `build_command_list` allows.
  * `POST /<repo>/git-receive-pack` -> parses the incoming command-list
    (`<old> <new> <ref>\\0caps`), RECORDS a receipt of the received push
    (`<repo>`, ref, old/new oid, PACK size) to stderr + an in-memory ledger,
    and returns a valid `application/x-git-receive-pack-result` report-status
    (`unpack ok` + per-ref `ok <ref>`) on the MAIN band (no side-band, matching
    the broker's `report-status agent=caprun` capability subset). Accepts a
    Basic-auth `x-access-token:<token>` credential (the git-over-HTTPS token
    convention the broker sets on the POST) but does NOT require it.

Repo-path routing (a test double, NOT a real git server — deliberately scoped so
it disturbs no prior test that relied on a push FAILING against this host):

  * first segment `accept` (e.g. `/accept/repo.git`) -> serve the advertisement
    and ACCEPT the receive-pack (the LEG-C clean-delivery success path).
  * first segment `redirect` (e.g. `/redirect/repo.git`) -> a 302 on info/refs,
    exercising the broker's redirect-none destination pin (LEG E): the frozen
    client refuses to follow it and the push fails closed.
  * any OTHER receive-pack repo (e.g. `/owner/repo.git`) -> 404 on info/refs, so
    a push there fails closed. This preserves the pre-existing Plan 44-04
    confirm-release unit tests (`clean_git_push_pending_row_is_confirm_releasable`,
    `git_push_confirm_releases_once_reaching_step7_dispatch`) that push to
    `/owner/repo.git` and assert `ConfirmedButSinkFailed` — they keep failing
    against the mock exactly as before, unweakened.

## v1.9 Phase 46 (G3): http.request.write POST /ingest

Additionally exposes a generic write sink so a CLEAN broker `http.request.write`
POST completes with a genuine 2xx over the SAME shipped egress path (validate_url
-> allowlist -> resolve-and-pin) unchanged, instead of 404ing for lack of a
generic write endpoint:

  * `POST /ingest` (query string ignored) -> RECORDS a receipt of the received
    write (`path`, `body_bytes`, credential presence) to a DISTINCT in-memory
    ledger `_WRITE_RECEIPTS` + a `mock-http-write: RECEIPT <json>` stderr line,
    and returns HTTP 201 with a minimal JSON acknowledgement (`received: true`
    + an opaque `id` — NO real GitHub data). A Basic-auth Authorization header
    is RECORDED (`authenticated`) but NOT required (mirrors the receive-pack
    mock). This is what LIVE-05's composed POST leg delivers to: a genuine 201
    surfaces broker-side as `http_write_succeeded`.

`/ingest` is routed as an ADDITIVE first-match case ahead of the pulls /
receive-pack branches; every pre-existing route (pulls 201, `/accept/*`
receive-pack, `/redirect/*` 302, the 404 fallbacks) is left byte-for-byte
intact. Stdlib-only, no new dependency.

NO third-party dependency and NO `git` binary: the mock parses the pkt-line
command-list in pure Python and returns a well-formed report-status, so it runs
unmodified on `python:3-slim` (honours CLAUDE.md "no new package-manager
dependency"). The receipt it records is what the acceptance test asserts on
(broker-side, via the resulting `git_push_succeeded` event — a valid report-
status is returned ONLY after the push is received + recorded).

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

# The all-zero SHA-1 object id. An empty-repo receive-pack advertisement carries
# exactly one "capabilities^{}" line pinned to this oid; the broker reads it as
# "the target ref is not advertised" -> a CREATE (zero old-oid, allowed).
ZERO_OID = "0" * 40


def _pkt(data: bytes) -> bytes:
    """Encode one git pkt-line: 4-hex big-endian length (over payload + the
    4-byte prefix) followed by the payload. Mirrors the broker's `pkt_line`."""
    return b"%04x" % (len(data) + 4) + data


# The pkt-line flush marker terminating a section.
FLUSH_PKT = b"0000"


def _read_pkt(buf: bytes, i: int):
    """Decode ONE pkt-line from `buf` at offset `i`. Returns `(payload_or_None,
    next_offset)` — `None` payload marks a flush. Fail-closed on a truncated
    header (returns `(None, len)` to stop). Pure, mirrors the broker decoder."""
    if i + 4 > len(buf):
        return None, len(buf)
    length = int(buf[i:i + 4], 16)
    if length == 0:
        return None, i + 4  # flush
    if length < 4 or i + length > len(buf):
        return None, len(buf)  # malformed / truncated -> stop
    return buf[i + 4:i + length], i + length


def _parse_first_command(body: bytes):
    """Extract `(old_oid, new_oid, refname)` from the FIRST receive-pack command
    pkt-line (`<old> <new> <ref>\\0caps`) at the front of the POST body. Returns
    `None` if no well-formed command line is present. Pure Python — no `git`."""
    payload, _ = _read_pkt(body, 0)
    if payload is None:
        return None
    # Strip the capability list after the first NUL, and a trailing LF.
    line = payload.split(b"\x00", 1)[0].rstrip(b"\n")
    parts = line.split(b" ")
    if len(parts) < 3:
        return None
    old_oid, new_oid, refname = parts[0], parts[1], parts[2]
    try:
        return old_oid.decode(), new_oid.decode(), refname.decode()
    except UnicodeDecodeError:
        return None


# In-memory receipt ledger: every accepted push appends a dict. The acceptance
# test asserts delivery broker-side (the `git_push_succeeded` event, produced
# ONLY after this ledger records + a valid report-status is returned); this
# ledger + the stderr log make the receipt independently observable in the
# sidecar output.
_RECEIPTS = []

# In-memory receipt ledger for `http.request.write` POST /ingest deliveries,
# kept DISTINCT from `_RECEIPTS` (git-push) so the two delivery surfaces stay
# independently observable. Each accepted /ingest POST appends a dict; the
# acceptance test asserts delivery broker-side (the `http_write_succeeded`
# event, produced ONLY after a genuine 201), this ledger + the stderr log make
# the receipt independently observable in the sidecar output.
_WRITE_RECEIPTS = []


def _is_pulls_path(path: str) -> bool:
    """True for `/repos/<owner>/<repo>/pulls` (ignoring any query string)."""
    path = path.split("?", 1)[0]
    parts = [p for p in path.split("/") if p]
    return len(parts) == 4 and parts[0] == "repos" and parts[3] == "pulls"


def _is_ingest_path(path: str) -> bool:
    """True for exactly `/ingest` (ignoring any query string)."""
    path = path.split("?", 1)[0]
    parts = [p for p in path.split("/") if p]
    return len(parts) == 1 and parts[0] == "ingest"


def _receive_pack_repo(path: str, suffix: str):
    """If `path` (query stripped) ends with `/<suffix>`, return the `<repo>` part
    (the leading path segments, slash-joined); else `None`. `suffix` is either
    `info/refs` or `git-receive-pack`."""
    path = path.split("?", 1)[0]
    marker = "/" + suffix
    if not path.endswith(marker):
        return None
    repo = path[: -len(marker)].strip("/")
    return repo or None


def _is_info_refs_receive_pack(path: str) -> bool:
    """True for `GET /<repo>/info/refs?service=git-receive-pack`."""
    query = path.split("?", 1)[1] if "?" in path else ""
    return (
        _receive_pack_repo(path, "info/refs") is not None
        and "service=git-receive-pack" in query
    )


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def _send(self, status: int, payload: dict) -> None:
        body = json.dumps(payload).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _send_raw(self, status: int, content_type: str, body: bytes) -> None:
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_POST(self) -> None:  # noqa: N802 (http.server naming)
        length = int(self.headers.get("Content-Length", "0") or "0")
        body = self.rfile.read(length) if length else b""
        repo = _receive_pack_repo(self.path, "git-receive-pack")
        if _is_ingest_path(self.path):
            self._handle_ingest(body)
        elif _is_pulls_path(self.path):
            # A plausible created-PR response: enough for the opaque success
            # event + CAS to be exercised. No real GitHub data.
            self._send(201, {
                "number": 1,
                "state": "open",
                "html_url": "https://github-mock.caprun.test/mock/mock/pull/1",
                "id": 1,
                "title": "mock",
            })
        elif repo is not None and repo.split("/", 1)[0] == "accept":
            self._handle_receive_pack(repo, body)
        elif repo is not None:
            # A receive-pack POST to a non-`accept` repo is refused (see the
            # module docstring's repo-path routing) — fail closed.
            self._send(404, {"message": "Not Found (mock git-receive-pack: only /accept/* repos accept a push)"})
        else:
            self._send(404, {"message": "Not Found (mock github: only POST /repos/*/pulls or /accept/*/git-receive-pack)"})

    def _handle_ingest(self, body: bytes) -> None:
        """Consume a generic `http.request.write` POST /ingest, RECORD a receipt,
        and return 201. The credential (Basic-auth Authorization header) is
        RECORDED but NOT required — mirrors the receive-pack mock."""
        has_auth = "Authorization" in self.headers
        receipt = {
            "path": self.path,
            "body_bytes": len(body),
            "authenticated": has_auth,
        }
        _WRITE_RECEIPTS.append(receipt)
        sys.stderr.write(
            "mock-http-write: RECEIPT " + json.dumps(receipt) + "\n"
        )
        self._send(201, {
            "id": len(_WRITE_RECEIPTS),
            "received": True,
        })

    def _handle_receive_pack(self, repo: str, body: bytes) -> None:
        """Consume a `git-receive-pack` command-list + PACK, RECORD a receipt,
        and return a clean report-status. The credential (Basic-auth
        `x-access-token:<token>`) is accepted but not required."""
        cmd = _parse_first_command(body)
        if cmd is None:
            # Fail-closed: a body with no well-formed command line is an unpack
            # error (the broker parses this as a push failure).
            report = _pkt(b"unpack ng no valid command line\n") + FLUSH_PKT
            self._send_raw(200, "application/x-git-receive-pack-result", report)
            return
        old_oid, new_oid, refname = cmd
        has_auth = self.headers.get("Authorization", "").startswith("Basic ")
        # The command-list is the pkt-line prefix; the PACK follows the flush.
        _, after_cmd = _read_pkt(body, 0)
        _, after_flush = _read_pkt(body, after_cmd)  # skip the command-list flush
        pack_len = max(0, len(body) - after_flush)
        receipt = {
            "repo": repo,
            "ref": refname,
            "old_oid": old_oid,
            "new_oid": new_oid,
            "pack_bytes": pack_len,
            "authenticated": has_auth,
        }
        _RECEIPTS.append(receipt)
        sys.stderr.write(
            "mock-git-receive-pack: RECEIPT " + json.dumps(receipt) + "\n"
        )
        # A clean report-status on the MAIN band (no side-band, matching the
        # broker's advertised capability subset): unpack ok + per-ref ok.
        report = (
            _pkt(b"unpack ok\n")
            + _pkt(b"ok " + refname.encode() + b"\n")
            + FLUSH_PKT
        )
        self._send_raw(200, "application/x-git-receive-pack-result", report)

    def do_GET(self) -> None:  # noqa: N802
        if _is_info_refs_receive_pack(self.path):
            repo = _receive_pack_repo(self.path, "info/refs")
            first_seg = repo.split("/", 1)[0] if repo else ""
            # LEG E (destination pin / redirect-none): a `redirect` repo returns a
            # 302 the broker's frozen client must REFUSE to follow — surfacing as
            # a non-success status, push fails closed.
            if first_seg == "redirect":
                self.send_response(302)
                self.send_header("Location", "https://github-mock.caprun.test/elsewhere/info/refs?service=git-receive-pack")
                self.send_header("Content-Length", "0")
                self.end_headers()
                return
            # Only an `accept` repo is advertised; any other repo 404s so a push
            # there fails closed (preserving the prior /owner/repo.git tests).
            if first_seg == "accept":
                self._send_receive_pack_advertisement()
                return
            self._send(404, {"message": "Not Found (mock git-receive-pack: only /accept/* repos are advertised)"})
            return
        self._send(404, {"message": "Not Found (mock github: only POST /repos/*/pulls or git smart-HTTP receive-pack)"})

    def _send_receive_pack_advertisement(self) -> None:
        """A valid empty-repo `git-receive-pack` info/refs advertisement: the
        `# service=git-receive-pack` announcement pkt + flush, then the single
        empty-repo `capabilities^{}` ref line (zero-oid), then a flush. The
        broker reads the un-advertised target ref as a CREATE (zero old-oid)."""
        body = _pkt(b"# service=git-receive-pack\n") + FLUSH_PKT
        # Empty-repo capabilities line: zero-oid + the "capabilities^{}"
        # sentinel refname, NUL-separated from the advertised capability set.
        # Advertise only `report-status` (main-band) — matching what the broker
        # both sends and expects (no side-band-64k).
        cap_line = ZERO_OID.encode() + b" capabilities^{}\x00report-status\n"
        body += _pkt(cap_line) + FLUSH_PKT
        self._send_raw(200, "application/x-git-receive-pack-advertisement", body)

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
