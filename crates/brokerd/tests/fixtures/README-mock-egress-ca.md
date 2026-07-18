# `mock-egress-ca.der` — test-only egress trust anchor

`mock-egress-ca.der` is a **self-signed test CA** for the DNS name
`github-mock.caprun.test`. It exists ONLY to let the Phase-40 composed
live-proof (Plan 40-03) reach a local TLS mock GitHub endpoint over REAL TLS
while riding the SHIPPED `validate_url` → allowlist → resolve-and-pin egress
path unchanged.

It is loaded into the broker egress `RootCertStore` **only** under the
non-default cargo feature `mock-egress-ca` (see `crates/brokerd/Cargo.toml`
`[features]` and `src/sinks/http_request.rs::egress_root_store`). With the
feature OFF (every release/default build), this anchor is absent and the
egress trust set is `webpki-roots` ONLY — asserted by
`egress_root_store_default_build_is_webpki_roots_only` in `http_request.rs`.

- No production trust: `.test` is a reserved non-resolvable TLD (RFC 6761); the
  cert is self-signed with no real CA in its chain.
- The matching **private key + PEM cert** are NOT checked in here — they ship
  with the mock server harness in Plan 40-03. Only the DER trust anchor lives
  in this fixtures dir.

## Reproduce (offline, openssl only — no Rust cert-gen dependency)

```sh
openssl req -x509 -newkey rsa:2048 -nodes \
  -keyout mock-egress-ca.key.pem \
  -out    mock-egress-ca.cert.pem \
  -days   36500 \
  -subj   "/CN=github-mock.caprun.test" \
  -addext "subjectAltName=DNS:github-mock.caprun.test" \
  -addext "basicConstraints=critical,CA:TRUE" \
  -addext "keyUsage=critical,keyCertSign,digitalSignature"

openssl x509 -in mock-egress-ca.cert.pem -outform DER -out mock-egress-ca.der
```

(The exact byte content is not reproducible run-to-run — a fresh keypair is
generated each time. Regenerating produces an equivalent, equally-valid test
anchor; the checked-in `.der` is one such instance.)
