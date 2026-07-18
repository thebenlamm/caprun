# `scripts/mock-github/` — mock GitHub HTTPS endpoint (Phase 40, LIVE-03)

A tiny **stdlib-only** Python HTTPS server (`server.py`) that stands in for
GitHub's write API during the composed live proof. It answers HTTP **201** with a
plausible created-PR JSON to `POST /repos/<owner>/<repo>/pulls`, and **404** to
everything else. It exists ONLY so a REAL `github.pr` POST completes over REAL TLS
while riding the SHIPPED broker egress path (`validate_url` → allowlist →
resolve-and-pin) unchanged (40-CONTEXT decisions 3 + 5).

- **No dependency:** uses only `http.server` + `ssl` from the Python standard
  library — runs on the pinned `python:3-slim` base with no `pip install`.
- **Driven by `scripts/compose-verify.sh`**, which mounts this dir into a sidecar
  and points the broker at it via `CAPRUN_GITHUB_API_BASE=https://github-mock.caprun.test`.

## Certificate (`certs/`)

`certs/github-mock.caprun.test.pem` (+ `.key`) is a **self-signed test cert** for
the DNS name `github-mock.caprun.test`. The **same certificate**, DER-encoded, is
the broker's feature-gated egress trust anchor at
`crates/brokerd/tests/fixtures/mock-egress-ca.der` — trusted ONLY under the
non-default `mock-egress-ca` cargo feature (Plan 40-02). So the mock's leaf cert
IS the anchor: the broker validates the mock over real TLS with the feature on,
and the mock host + cert are unreachable in any default/release build.

- **Test-only, no production trust:** `.test` is a reserved non-resolvable TLD
  (RFC 6761); the cert is self-signed with no real CA in its chain and covers a
  non-real domain. The private key is checked in deliberately — it has no
  production value.
- The cert carries `CA:FALSE` + `keyUsage=digitalSignature` +
  `extendedKeyUsage=serverAuth`, so rustls/webpki accepts it BOTH as the trust
  anchor (rustls `RootCertStore::add` trusts an explicitly-provided cert
  regardless of the CA bit) AND as a server end-entity cert for
  `github-mock.caprun.test`. Two rustls-webpki requirements the Plan 40-04
  composed live proof caught the hard way (openssl does NOT enforce either, so
  `openssl s_client` verified OK while rustls rejected the handshake as "error
  sending request"):
    - `basicConstraints=CA:FALSE` is REQUIRED — a `CA:TRUE` cert presented as
      the server leaf is rejected with `CaUsedAsEndEntity` (an end-entity must
      not be a CA).
    - `extendedKeyUsage=serverAuth` is REQUIRED — rustls-webpki verifies the
      leaf with `KeyUsage::server_auth()` and rejects a cert that does not
      assert it (`RequiredEkuNotFound`).

### Reproduce (offline, openssl only — no Rust cert-gen dependency)

This is the **exact** command from `crates/brokerd/tests/fixtures/README-mock-egress-ca.md`.
The `.pem`/`.key` here and the `.der` anchor there encode the SAME certificate —
regenerate all three together (a fresh keypair each run; the checked-in set is one
such instance):

```sh
# from scripts/mock-github/certs/
openssl req -x509 -newkey rsa:2048 -nodes \
  -keyout github-mock.caprun.test.key \
  -out    github-mock.caprun.test.pem \
  -days   36500 \
  -subj   "/CN=github-mock.caprun.test" \
  -addext "subjectAltName=DNS:github-mock.caprun.test" \
  -addext "basicConstraints=critical,CA:FALSE" \
  -addext "keyUsage=critical,digitalSignature" \
  -addext "extendedKeyUsage=serverAuth"

# derive the broker's DER trust anchor from the SAME cert:
openssl x509 -in github-mock.caprun.test.pem -outform DER \
  -out ../../../crates/brokerd/tests/fixtures/mock-egress-ca.der
```

Verify the PEM cert and the DER anchor are the same certificate:

```sh
openssl x509 -in github-mock.caprun.test.pem -noout -fingerprint -sha256
openssl x509 -inform DER -in ../../../crates/brokerd/tests/fixtures/mock-egress-ca.der \
  -noout -fingerprint -sha256   # must print the identical SHA-256 fingerprint
```
