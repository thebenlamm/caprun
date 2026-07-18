//! sinks/http_request — the broker-side, read-only `http.request` GET egress
//! (HTTP-01/HTTP-03, DESIGN §3.1 Pattern A).
//!
//! # Security role (Pattern A, broker-resident — NEVER the confined worker)
//!
//! This is the ONLY code path that performs an outbound HTTP GET, and it is
//! INVOKED only in broker-resident code, exactly like `email_smtp.rs`'s SMTP
//! call. The confined worker cannot perform this GET not because these deps are
//! absent from its binary image (they are linked in via `cli/caprun` →
//! `brokerd`) but because it runs under a KERNEL-ENFORCED default-deny network
//! sandbox — the real boundary is kernel net-deny, not the dependency graph
//! (FIX 6; see `Cargo.toml`).
//!
//! # SSRF resolve-and-pin (DESIGN §3.6, threats T-37-01/03/04)
//!
//! The fetch is defended in depth, all before/around the single socket:
//!   1. `validate_url` — reject `userinfo@`, any non-`https` scheme, and
//!      IP-encoding tricks (decimal/octal/hex/plain IP-literal hosts); only a
//!      DNS domain host survives.
//!   2. host allowlist — the domain MUST be on the hardcoded `HOST_ALLOWLIST`,
//!      checked BEFORE any DNS resolve (fail-closed, DESIGN §8).
//!   3. `ssrf_check` — every resolved IP is classified; loopback / RFC1918 /
//!      link-local (incl. cloud-metadata) / CGNAT / ULA / IPv6-mapped-v4 /
//!      unspecified are denied. A mixed DNS answer denies the whole request.
//!   4. resolve-and-pin — reqwest connects to the SAME SSRF-vetted IP via
//!      `.resolve(host, pinned)` (SNI/Host = original hostname), so the checked
//!      IP equals the connected IP (DNS-rebind TOCTOU close), with redirect
//!      following DISABLED (a 30x cannot bounce to a denied range).
//!
//! # NO mint / NO demotion here (DESIGN §10, Gate 3)
//!
//! This module performs NO `ValueStore::mint`, appends NO audit `Event`, and
//! never touches session status — Plan 03 (`server.rs` Allowed-GET dispatch)
//! owns the `mint_from_http` genesis + I1 demotion. Keeping this module free of
//! any mint token is what keeps it out of `check-invariants.sh` Gate 3's
//! mint-site restriction.
//!
//! # TLS trust anchors (DESIGN §5.1/§5.2)
//!
//! The reqwest client is built with a preconfigured rustls `ClientConfig` using
//! the pure-Rust `ring` crypto provider and the compiled-in `webpki-roots`
//! trust anchors — so validation needs no `SSL_CERT_*` env var or system cert
//! store (`env_clear()`-hermetic). We deliberately do NOT use reqwest's default
//! `rustls` feature (it pulls the aws-lc-rs C provider) or
//! rustls-platform-verifier (system store).
//!
//! # `mock-egress-ca` feature (NON-DEFAULT, test-only — LIVE-03/T-40-03)
//!
//! The DEFAULT/RELEASE build trusts EXACTLY the `webpki-roots` anchors and
//! allowlists EXACTLY `api.github.com`. Under the NON-DEFAULT `mock-egress-ca`
//! cargo feature (Plan 40-03 compose-verify harness ONLY) the egress path
//! ADDITIONALLY trusts ONE checked-in self-signed test CA
//! (`tests/fixtures/mock-egress-ca.der`) and ADDITIONALLY allowlists ONE test
//! host (`github-mock.caprun.test`) — so the composed live-proof can POST to a
//! local TLS mock over REAL TLS while riding the SAME `validate_url` →
//! allowlist → `resolve_and_pin` path. The feature adds ONLY that one anchor +
//! one host; it NEVER relaxes https-only, `ssrf_check`, the IP pin, or
//! redirect-none, and both additions are ABSENT from every production build.
//! The closing fresh adversarial code-trace MUST confirm the release trust set
//! is unchanged — see the `not(mock-egress-ca)` invariant tests below.
use anyhow::{bail, Result};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

/// Connect timeout bounding the TCP+TLS handshake (FIX 3, DoS — a hung/black-holed
/// endpoint must not pin a broker task forever).
const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Total-request timeout bounding the whole GET incl. body read (FIX 3, DoS —
/// a slow-drip response body must not stall the broker indefinitely).
const HTTP_TOTAL_TIMEOUT: Duration = Duration::from_secs(30);

/// Hard cap on the response body we will buffer (FIX 3, DoS). Mirrors
/// `process_exec::MAX_COMBINED_OUTPUT_BYTES`'s fail-closed discipline: exceeding
/// this stops the read and errors — NEVER a silent truncate-and-keep-going.
// Referenced by the Linux `do_pinned_get` streaming loop and by host-portable
// unit tests; on a non-test macOS build (stub `do_pinned_get`) it is unreferenced.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
const MAX_RESPONSE_BODY_BYTES: usize = 10 * 1024 * 1024;

/// Fail-closed body-cap check over a running byte total (FIX 3). Pure,
/// host-portable — the same predicate the Linux streaming read applies after
/// each chunk. Mirrors `process_exec::read_capped`: over the cap is an `Err`,
/// never a truncation.
// Consumed by the Linux `do_pinned_get` streaming loop and by a host-portable
// unit test; on a non-test macOS build (stub `do_pinned_get`) it is unreferenced.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn check_body_cap(total: usize, cap: usize) -> Result<()> {
    if total > cap {
        bail!("http.request: response body exceeded the {cap}-byte cap (fail-closed)");
    }
    Ok(())
}

/// The fetch-target host allowlist (DESIGN §3.6 + §11 — an operator-surfaced
/// deployment CONSTANT, never runtime-configurable from a plan node /
/// `ValueNode` / audit DB). This is a security property (SSRF/egress bound),
/// mirroring `email_smtp.rs`'s broker-owned D-04 trusted endpoint config: it is
/// broker-local trusted config, NOT a swappable policy file.
const HOST_ALLOWLIST: &[&str] = &["api.github.com"];

/// NON-DEFAULT `mock-egress-ca` feature ONLY: the single extra test host the
/// Plan 40-03 composed live-proof POSTs to over the local TLS mock. Compiled
/// OUT of — and thus absent from — every production/default build; the release
/// allowlist is `HOST_ALLOWLIST` (= `["api.github.com"]`) ONLY.
#[cfg(feature = "mock-egress-ca")]
const MOCK_EGRESS_HOST: &str = "github-mock.caprun.test";

/// True iff `host` is on the hardcoded allowlist (case-insensitive). A
/// non-allowlisted host is rejected by `invoke_http_get` BEFORE any DNS
/// resolve. Pure, host-portable.
///
/// The default/release allowlist is `HOST_ALLOWLIST` (= `["api.github.com"]`)
/// ONLY. Under the NON-DEFAULT `mock-egress-ca` feature a SINGLE extra host
/// (`MOCK_EGRESS_HOST`) is additionally accepted — the ONLY egress relaxation
/// the feature makes. It does NOT touch `validate_url` (https-only),
/// `ssrf_check`, the IP pin, or redirect-none.
fn is_host_allowlisted(host: &str) -> bool {
    if HOST_ALLOWLIST.iter().any(|allowed| allowed.eq_ignore_ascii_case(host)) {
        return true;
    }
    #[cfg(feature = "mock-egress-ca")]
    if MOCK_EGRESS_HOST.eq_ignore_ascii_case(host) {
        return true;
    }
    false
}

/// Validate a fetch URL and return its DNS hostname. Rejects a `userinfo@`
/// component, any scheme other than `https`, and IP-encoding tricks
/// (decimal/octal/hex-packed or plain IP-literal hosts — the WHATWG URL parser
/// normalizes those to an `Ipv4`/`Ipv6` host, which we reject: only a DNS
/// domain host is allowed). Pure, host-portable.
fn validate_url(url: &str) -> Result<String> {
    let parsed = reqwest::Url::parse(url).map_err(|e| anyhow::anyhow!("invalid URL: {e}"))?;

    if parsed.scheme() != "https" {
        bail!("http.request: only https is allowed, got scheme {:?}", parsed.scheme());
    }

    // Reject any userinfo (`user@` or `user:pass@`) — SSRF/credential smuggling.
    if !parsed.username().is_empty() || parsed.password().is_some() {
        bail!("http.request: URL userinfo component is not allowed");
    }

    // Reject any explicit port — https implies 443 (FIX 2, T-37-03 port pin).
    // The resolve-and-pin `.resolve(host, socket_addr)` pins only the IP; reqwest
    // still connects on the URL's *port*, so an unconstrained port means the
    // SSRF-vetted endpoint and the connected endpoint could differ in the port
    // dimension (e.g. an allowlisted host on an internal-only admin port). Pin
    // to the default 443 by requiring the URL carry no explicit port.
    if parsed.port().is_some() {
        bail!("http.request: explicit port is not allowed (https implies 443)");
    }

    // The WHATWG URL parser normalizes decimal/octal/hex-packed and plain IP
    // literals into a typed `Host::Ipv4`/`Host::Ipv6`; only a DNS `Domain` host
    // is accepted — this rejects every IP-encoding trick in one check.
    match parsed.host() {
        Some(url::Host::Domain(d)) if !d.is_empty() => Ok(d.to_string()),
        Some(url::Host::Domain(_)) => bail!("http.request: empty host"),
        Some(url::Host::Ipv4(_)) | Some(url::Host::Ipv6(_)) => {
            bail!("http.request: IP-literal / IP-encoded hosts are not allowed (use a DNS name)")
        }
        None => bail!("http.request: URL has no host"),
    }
}

/// The load-bearing SSRF classifier (DESIGN §3.6). Returns `Err` for a resolved
/// IP in any denied range: loopback, RFC1918, link-local (incl. cloud-metadata
/// 169.254.169.254), CGNAT, ULA, IPv6-mapped-IPv4 (embedded v4 re-checked),
/// unspecified. `Ok` for an ordinary public IP. Pure over `IpAddr`,
/// host-portable — the same check that runs on the IP reqwest is then pinned to.
pub fn ssrf_check(ip: IpAddr) -> Result<()> {
    let denied = match ip {
        IpAddr::V4(v4) => ipv4_denied(v4),
        IpAddr::V6(v6) => ipv6_denied(v6),
    };
    if denied {
        bail!("http.request: resolved IP {ip} is in a denied SSRF range");
    }
    Ok(())
}

/// DESIGN §3.6 IPv4 denials: loopback (127/8), RFC1918 (10/8, 172.16/12,
/// 192.168/16), link-local (169.254/16, incl. cloud-metadata 169.254.169.254),
/// CGNAT (100.64/10), unspecified (0.0.0.0), broadcast (255.255.255.255).
///
/// FIX 5 additions (defense-depth ranges that must never be a fetch target):
/// multicast (224.0.0.0/4), reserved (240.0.0.0/4), benchmark (198.18.0.0/15),
/// and IETF-protocol-assignments (192.0.0.0/24 — incl. the 192.0.0.170 NAT64
/// discovery specials).
fn ipv4_denied(ip: Ipv4Addr) -> bool {
    ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || ip.is_multicast()
        || is_cgnat(ip)
        || is_reserved_240(ip)
        || is_benchmark(ip)
        || is_ietf_protocol(ip)
}

/// 100.64.0.0/10 (RFC 6598 carrier-grade NAT).
fn is_cgnat(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    o[0] == 100 && (0x40..=0x7f).contains(&o[1])
}

/// 240.0.0.0/4 (RFC 1112 §4 reserved / "future use"). FIX 5.
fn is_reserved_240(ip: Ipv4Addr) -> bool {
    ip.octets()[0] >= 240
}

/// 198.18.0.0/15 (RFC 2544 benchmarking). FIX 5.
fn is_benchmark(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    o[0] == 198 && (o[1] == 18 || o[1] == 19)
}

/// 192.0.0.0/24 (RFC 6890 IETF protocol assignments). FIX 5.
fn is_ietf_protocol(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    o[0] == 192 && o[1] == 0 && o[2] == 0
}

/// DESIGN §3.6 IPv6 denials: loopback (::1), unspecified (::), ULA (fc00::/7),
/// link-local (fe80::/10), and IPv6-mapped-IPv4 (::ffff:0:0/96) whose embedded
/// v4 is re-checked against the v4 ranges.
///
/// FIX 5 additions: transition/embedding mechanisms an attacker can point at an
/// internal host — NAT64 well-known prefix (64:ff9b::/96), 6to4 (2002::/16),
/// Teredo (2001:0::/32) are denied wholesale; the deprecated IPv4-compatible
/// form (::a.b.c.d, first 96 bits zero) has its embedded v4 re-checked.
fn ipv6_denied(ip: Ipv6Addr) -> bool {
    // Handle the ::/:: 1 specials first so the IPv4-compatible re-check below
    // does not have to special-case them.
    if ip.is_loopback() || ip.is_unspecified() {
        return true;
    }
    // IPv4-mapped (::ffff:a.b.c.d): re-check the embedded v4.
    if let Some(v4) = ip.to_ipv4_mapped() {
        return ipv4_denied(v4);
    }
    // Transition mechanisms — denied wholesale (github is never reached via any
    // of these, and each embeds an attacker-influenced v4).
    if is_nat64_wkp(ip) || is_6to4(ip) || is_teredo(ip) {
        return true;
    }
    // Deprecated IPv4-compatible (::a.b.c.d): first 96 bits zero, embedded v4
    // re-checked (catches ::10.0.0.1, ::169.254.169.254, etc.).
    if let Some(v4) = ipv4_compatible_embedded(ip) {
        return ipv4_denied(v4);
    }
    is_ula(ip) || is_v6_link_local(ip)
}

/// fc00::/7 (RFC 4193 unique local address).
fn is_ula(ip: Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xfe00) == 0xfc00
}

/// fe80::/10 (link-local unicast).
fn is_v6_link_local(ip: Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xffc0) == 0xfe80
}

/// 64:ff9b::/96 (RFC 6052 NAT64 well-known prefix). FIX 5.
fn is_nat64_wkp(ip: Ipv6Addr) -> bool {
    let s = ip.segments();
    s[0] == 0x0064 && s[1] == 0xff9b && s[2] == 0 && s[3] == 0 && s[4] == 0 && s[5] == 0
}

/// 2002::/16 (RFC 3056 6to4). FIX 5.
fn is_6to4(ip: Ipv6Addr) -> bool {
    ip.segments()[0] == 0x2002
}

/// 2001:0::/32 (RFC 4380 Teredo). FIX 5.
fn is_teredo(ip: Ipv6Addr) -> bool {
    let s = ip.segments();
    s[0] == 0x2001 && s[1] == 0x0000
}

/// Deprecated IPv4-compatible IPv6 (::a.b.c.d — first 96 bits zero): returns the
/// embedded v4 to re-check. Distinct from IPv4-mapped (::ffff:a.b.c.d, seg[5] =
/// 0xffff, handled by `to_ipv4_mapped`). The ::/::1 specials are handled by the
/// caller before this runs, so a Some here is a genuine embedded v4. FIX 5.
fn ipv4_compatible_embedded(ip: Ipv6Addr) -> Option<Ipv4Addr> {
    let s = ip.segments();
    if s[0..6].iter().all(|&x| x == 0) {
        Some(Ipv4Addr::new(
            (s[6] >> 8) as u8,
            (s[6] & 0xff) as u8,
            (s[7] >> 8) as u8,
            (s[7] & 0xff) as u8,
        ))
    } else {
        None
    }
}

/// Vet a set of resolved socket addresses and return the one to pin. Fail-closed:
/// if the set is empty, or if ANY resolved IP is in a denied SSRF range, returns
/// `Err` (a mixed good/bad DNS answer denies the whole request). Pure over the
/// resolved list (no DNS) → host-portable and unit-testable.
// Consumed by the Linux `do_pinned_get` and by host-portable unit tests; on a
// non-test macOS build (stub `do_pinned_get`) it is unreferenced.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn vet_resolved(addrs: &[SocketAddr]) -> Result<SocketAddr> {
    if addrs.is_empty() {
        bail!("http.request: host resolved to no addresses");
    }
    // Fail-closed: EVERY resolved IP must pass ssrf_check. A mixed answer
    // (one public + one internal) denies the whole request.
    for a in addrs {
        ssrf_check(a.ip())?;
    }
    Ok(addrs[0])
}

/// Build the redirect-free, IP-pinned reqwest client for a vetted destination.
/// Host-portable (no network): compiles + type-checks the ring/webpki-roots TLS
/// wiring on macOS. `redirect(Policy::none())` (T-37-04) + `.resolve(host,
/// pinned)` pins the connect target to the SSRF-vetted IP (SNI/Host = original
/// hostname), closing the DNS-rebind TOCTOU (T-37-03).
// Consumed by the Linux `do_pinned_get` and by a host-portable unit test; on a
// non-test macOS build it is unreferenced.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn build_pinned_client(host: &str, pinned: SocketAddr) -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .use_preconfigured_tls(ring_webpki_tls_config())
        .resolve(host, pinned)
        // FIX 3 (DoS): bound the handshake and the whole request. The response
        // body is additionally byte-capped by the streaming read in do_pinned_get.
        .connect_timeout(HTTP_CONNECT_TIMEOUT)
        .timeout(HTTP_TOTAL_TIMEOUT)
        .build()
        .map_err(|e| anyhow::anyhow!("http.request: failed to build client: {e}"))
}

/// Build the broker egress TLS trust anchor set.
///
/// The DEFAULT / RELEASE build trusts EXACTLY the compiled-in `webpki-roots`
/// anchors (DESIGN §5.2) — nothing else, byte-for-byte the Phase-38 trust set.
///
/// Under the NON-DEFAULT `mock-egress-ca` feature (Plan 40-03 compose-verify
/// harness ONLY, `--features mock-egress-ca`) a SINGLE checked-in self-signed
/// test CA (`tests/fixtures/mock-egress-ca.der`, `github-mock.caprun.test`) is
/// ALSO added, so the composed live-proof can reach a local TLS mock over REAL
/// TLS while riding the SAME `validate_url` → allowlist → `resolve_and_pin`
/// path. The feature adds ONLY this one trust anchor; it NEVER relaxes
/// https-only (`validate_url`), the SSRF classifier (`ssrf_check`), the IP pin
/// (`resolve_and_pin`/`vet_resolved`), or redirect-none. The mock anchor is
/// ABSENT from every production/default build.
///
/// SECURITY (T-40-03): the closing fresh adversarial code-trace MUST confirm
/// the release (no-feature) trust set is webpki-roots ONLY — see
/// `egress_root_store_default_build_is_webpki_roots_only`.
// Consumed transitively via `ring_webpki_tls_config` → `build_pinned_client`,
// plus the host-portable unit tests; same platform-gated
// unreferenced-on-macOS-non-test situation.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn egress_root_store() -> rustls::RootCertStore {
    // `mut` is used ONLY under the mock-egress-ca feature (the `roots.add`
    // below); it is genuinely unused in the default build.
    // `mut` is used ONLY under the mock-egress-ca feature (the `roots.add`
    // below); it is genuinely unused in the default/release build.
    #[allow(unused_mut)]
    let mut roots = rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };
    // NON-DEFAULT feature ONLY: add the single test CA trust anchor. Gated so
    // it is compiled OUT of — and thus absent from — every production/default
    // build. No PEM parser, no new dependency: the DER is embedded verbatim.
    #[cfg(feature = "mock-egress-ca")]
    {
        let der = rustls::pki_types::CertificateDer::from(
            &include_bytes!("../../tests/fixtures/mock-egress-ca.der")[..],
        );
        roots
            .add(der)
            .expect("mock-egress-ca: checked-in test CA is a valid DER trust anchor");
    }
    roots
}

/// Preconfigured rustls `ClientConfig`: pure-Rust `ring` provider (DESIGN §5.1)
/// + the `egress_root_store` trust anchors (DESIGN §5.2 — `env_clear()`
/// hermetic, no system cert store / SSL_CERT_* / platform verifier).
// Consumed transitively via `build_pinned_client`; same platform-gated
// unreferenced-on-macOS-non-test situation.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn ring_webpki_tls_config() -> rustls::ClientConfig {
    let roots = egress_root_store();
    rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .expect("ring provider supports rustls default protocol versions")
        .with_root_certificates(roots)
        .with_no_client_auth()
}

/// Broker-side read-only GET egress. Order: validate_url → allowlist gate
/// (Err BEFORE any resolve) → [Linux] resolve → vet all resolved IPs
/// (`ssrf_check`) → pin to the vetted IP → GET with redirects OFF → body text.
///
/// This function performs NO minting, appends NO audit event, and does NOT
/// touch session status — Plan 03 owns all of that (keeps this module out of
/// Gate 3's mint-site restriction, DESIGN §10).
pub async fn invoke_http_get(url: &str) -> Result<String> {
    let host = validate_url(url)?;
    // Allowlist gate — BEFORE any DNS resolve or socket (DESIGN §8 fail-closed).
    if !is_host_allowlisted(&host) {
        bail!("http.request: host {host:?} is not on the allowlist");
    }
    do_pinned_get(url, &host).await
}

/// Linux-only: resolve `host` on a blocking thread, vet the WHOLE resolved set
/// (`vet_resolved` → `ssrf_check`, fail-closed on any denied IP), and build the
/// redirect-free client pinned to the vetted IP. This is the shared §3.6
/// resolve-and-pin core reused by BOTH `do_pinned_get` and `do_pinned_post` —
/// the classifiers (`ssrf_check`, range predicates) are NEVER re-implemented,
/// only invoked here. The resolved IPs are the EXACT set vetted and pinned (no
/// re-resolve later — DNS-rebind TOCTOU close).
#[cfg(target_os = "linux")]
async fn resolve_and_pin(host: &str) -> Result<reqwest::Client> {
    use std::net::ToSocketAddrs;

    let host_owned = host.to_string();
    let addrs: Vec<SocketAddr> = tokio::task::spawn_blocking(move || {
        (host_owned.as_str(), 443u16)
            .to_socket_addrs()
            .map(|it| it.collect::<Vec<_>>())
    })
    .await
    .map_err(|e| anyhow::anyhow!("http.request: resolver task join error: {e}"))?
    .map_err(|e| anyhow::anyhow!("http.request: DNS resolution failed: {e}"))?;

    let pinned = vet_resolved(&addrs)?;
    build_pinned_client(host, pinned)
}

/// Linux: resolve → vet all resolved IPs → pin to the vetted IP → redirect-free
/// GET → response body text. The real DNS-resolve + socket-connect leg is
/// Linux-gated per the project's Linux-only pattern (CLAUDE.md); live-HTTPS
/// behavior is deferred to Phase 40.
#[cfg(target_os = "linux")]
async fn do_pinned_get(url: &str, host: &str) -> Result<String> {
    let client = resolve_and_pin(host).await?;
    let mut resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("http.request: GET failed: {e}"))?;

    // Stream the body chunk-by-chunk with a fail-closed byte cap (FIX 3, DoS) —
    // NOT resp.text(), which would buffer an unbounded body into memory first.
    // Exceeding MAX_RESPONSE_BODY_BYTES stops the read and errors; the client's
    // total timeout (build_pinned_client) is the separate slow-drip backstop.
    let mut body: Vec<u8> = Vec::new();
    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| anyhow::anyhow!("http.request: reading response body failed: {e}"))?
    {
        body.extend_from_slice(&chunk);
        check_body_cap(body.len(), MAX_RESPONSE_BODY_BYTES)?;
    }
    Ok(String::from_utf8_lossy(&body).into_owned())
}

/// Non-Linux (dev macOS) no-op stub: the pure classifiers + client-build wiring
/// above are host-portable and fully tested here, but the real socket leg is
/// Linux-only (CLAUDE.md); live-HTTPS behavior is deferred to Phase 40.
#[cfg(not(target_os = "linux"))]
async fn do_pinned_get(_url: &str, _host: &str) -> Result<String> {
    bail!("http.request live GET is Linux-only (macOS no-op stub); deferred to Phase 40")
}

/// Broker-side WRITE egress: one authenticated `POST` to a pinned, allowlisted
/// destination — the sole network leg the `github.pr` sink (GITHUB-01) needs.
///
/// Order is IDENTICAL to `invoke_http_get` (DESIGN §3.6): `validate_url` →
/// allowlist gate (Err BEFORE any resolve) → [Linux] the SAME
/// resolve→`vet_resolved`→`build_pinned_client` pin path the GET uses
/// (`resolve_and_pin`, no classifier is re-implemented) → redirect-free POST →
/// `(status_code, body_text)`. The `bearer` token is set via `.bearer_auth` and
/// the `json_body` (already serialized by the caller — reqwest's `json` feature
/// is deliberately NOT enabled) is sent verbatim. redirect(none), the
/// connect/total timeouts, and the response-body byte cap are the SAME as the
/// GET path.
///
/// SSRF pin note (MAJOR-4): the destination host is whatever `url`'s host is,
/// and it MUST be on `HOST_ALLOWLIST` — so a `github.pr` caller that builds its
/// URL from a FIXED `api_base` (never from owner/repo) cannot have an
/// attacker-influenced arg redirect the POST to a different host.
///
/// Like `invoke_http_get`, this performs NO minting, appends NO audit event, and
/// never touches session status (keeps this module out of Gate 3's mint-site
/// restriction).
pub(crate) async fn invoke_pinned_post(
    url: &str,
    bearer: &str,
    json_body: &str,
) -> Result<(u16, String)> {
    let host = validate_url(url)?;
    // Allowlist gate — BEFORE any DNS resolve or socket (DESIGN §8 fail-closed).
    if !is_host_allowlisted(&host) {
        bail!("http.request: host {host:?} is not on the allowlist");
    }
    do_pinned_post(url, &host, bearer, json_body).await
}

/// Linux: resolve-and-pin (shared with the GET) → redirect-free authenticated
/// POST → `(status, capped body text)`. Live-HTTPS behavior is deferred to
/// Phase 40 (mock endpoint). Redirect following is DISABLED (a 30x cannot bounce
/// the write to a denied range), and the response body is byte-capped by the
/// SAME fail-closed streaming read as the GET.
#[cfg(target_os = "linux")]
async fn do_pinned_post(
    url: &str,
    host: &str,
    bearer: &str,
    json_body: &str,
) -> Result<(u16, String)> {
    let client = resolve_and_pin(host).await?;
    let mut resp = client
        .post(url)
        .bearer_auth(bearer)
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "caprun")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header("Content-Type", "application/json")
        .body(json_body.to_string())
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("http.request: POST failed: {e}"))?;

    let status = resp.status().as_u16();

    // Stream the response body with the SAME fail-closed byte cap as the GET
    // (FIX 3, DoS) — never resp.text() (unbounded buffer).
    let mut body: Vec<u8> = Vec::new();
    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| anyhow::anyhow!("http.request: reading POST response body failed: {e}"))?
    {
        body.extend_from_slice(&chunk);
        check_body_cap(body.len(), MAX_RESPONSE_BODY_BYTES)?;
    }
    Ok((status, String::from_utf8_lossy(&body).into_owned()))
}

/// Non-Linux (dev macOS) no-op stub — mirrors `do_pinned_get`'s cfg split: the
/// pure gates (`validate_url`, allowlist, `build_pinned_client` wiring) are
/// host-portable and tested, but the real socket leg is Linux-only (CLAUDE.md);
/// live GitHub behavior is deferred to Phase 40 (mock endpoint).
#[cfg(not(target_os = "linux"))]
async fn do_pinned_post(
    _url: &str,
    _host: &str,
    _bearer: &str,
    _json_body: &str,
) -> Result<(u16, String)> {
    bail!("github.pr live POST is Linux-only (macOS no-op stub); deferred to Phase 40")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    fn v4(s: &str) -> IpAddr {
        IpAddr::V4(s.parse::<Ipv4Addr>().unwrap())
    }
    fn v6(s: &str) -> IpAddr {
        IpAddr::V6(s.parse::<Ipv6Addr>().unwrap())
    }

    // ---- ssrf_check: each DESIGN §3.6 denied range individually ----

    #[test]
    fn ssrf_denies_loopback_v4() {
        assert!(ssrf_check(v4("127.0.0.1")).is_err());
        assert!(ssrf_check(v4("127.13.37.1")).is_err()); // whole 127/8
    }

    #[test]
    fn ssrf_denies_loopback_v6() {
        assert!(ssrf_check(v6("::1")).is_err());
    }

    #[test]
    fn ssrf_denies_rfc1918() {
        assert!(ssrf_check(v4("10.0.0.1")).is_err());
        assert!(ssrf_check(v4("172.16.0.1")).is_err());
        assert!(ssrf_check(v4("172.31.255.255")).is_err());
        assert!(ssrf_check(v4("192.168.1.1")).is_err());
    }

    #[test]
    fn ssrf_denies_link_local() {
        assert!(ssrf_check(v4("169.254.1.1")).is_err());
    }

    #[test]
    fn ssrf_denies_cloud_metadata() {
        // 169.254.169.254 — the canonical cloud IMDS endpoint.
        assert!(ssrf_check(v4("169.254.169.254")).is_err());
    }

    #[test]
    fn ssrf_denies_cgnat() {
        assert!(ssrf_check(v4("100.64.0.1")).is_err());
        assert!(ssrf_check(v4("100.127.255.255")).is_err());
        // boundary: 100.63.x and 100.128.x are OUTSIDE 100.64/10 → public
        assert!(ssrf_check(v4("100.63.255.255")).is_ok());
        assert!(ssrf_check(v4("100.128.0.1")).is_ok());
    }

    #[test]
    fn ssrf_denies_v6_link_local() {
        assert!(ssrf_check(v6("fe80::1")).is_err());
    }

    #[test]
    fn ssrf_denies_ula() {
        assert!(ssrf_check(v6("fc00::1")).is_err());
        assert!(ssrf_check(v6("fd12:3456::1")).is_err()); // fc00::/7 covers fd..
    }

    #[test]
    fn ssrf_denies_ipv6_mapped_v4() {
        // ::ffff:0:0/96 embedding a denied v4 → denied via embedded re-check.
        assert!(ssrf_check(v6("::ffff:127.0.0.1")).is_err());
        assert!(ssrf_check(v6("::ffff:169.254.169.254")).is_err());
        assert!(ssrf_check(v6("::ffff:10.0.0.1")).is_err());
    }

    #[test]
    fn ssrf_denies_unspecified() {
        assert!(ssrf_check(v4("0.0.0.0")).is_err());
        assert!(ssrf_check(v6("::")).is_err());
    }

    #[test]
    fn ssrf_allows_public_ip() {
        assert!(ssrf_check(v4("140.82.112.3")).is_ok()); // github.com public
        assert!(ssrf_check(v4("8.8.8.8")).is_ok());
        assert!(ssrf_check(v6("2606:50c0:8000::153")).is_ok()); // public v6
    }

    // ---- FIX 5: additional v4 ranges, with boundaries ----

    #[test]
    fn ssrf_denies_v4_multicast() {
        assert!(ssrf_check(v4("224.0.0.1")).is_err());
        assert!(ssrf_check(v4("239.255.255.255")).is_err());
        // boundary: 223.x is below 224/4 → public; 240.x is reserved (next test)
        assert!(ssrf_check(v4("223.255.255.255")).is_ok());
    }

    #[test]
    fn ssrf_denies_v4_reserved_240() {
        assert!(ssrf_check(v4("240.0.0.1")).is_err());
        assert!(ssrf_check(v4("254.254.254.254")).is_err());
    }

    #[test]
    fn ssrf_denies_v4_benchmark_198_18() {
        assert!(ssrf_check(v4("198.18.0.1")).is_err());
        assert!(ssrf_check(v4("198.19.255.255")).is_err());
        // boundary: 198.17.x and 198.20.x are OUTSIDE 198.18/15 → public
        assert!(ssrf_check(v4("198.17.255.255")).is_ok());
        assert!(ssrf_check(v4("198.20.0.1")).is_ok());
    }

    #[test]
    fn ssrf_denies_v4_ietf_protocol_192_0_0() {
        assert!(ssrf_check(v4("192.0.0.1")).is_err());
        assert!(ssrf_check(v4("192.0.0.170")).is_err()); // NAT64 discovery special
        assert!(ssrf_check(v4("192.0.0.255")).is_err());
        // boundary: 192.0.1.x (192.0.1/24 docs range) is NOT 192.0.0/24 → public
        assert!(ssrf_check(v4("192.0.1.1")).is_ok());
    }

    // ---- FIX 5: additional v6 ranges, with boundaries ----

    #[test]
    fn ssrf_denies_v6_nat64_wkp() {
        // 64:ff9b::/96 — deny wholesale, incl. one embedding an internal v4.
        assert!(ssrf_check(v6("64:ff9b::1")).is_err());
        assert!(ssrf_check(v6("64:ff9b::a00:1")).is_err()); // embeds 10.0.0.1
        // boundary: 64:ff9b:1:: is the /48 local-use prefix, NOT the /96 WKP →
        // not matched by is_nat64_wkp (documents the /96 boundary).
        assert!(ssrf_check(v6("64:ff9b:1::1")).is_ok());
    }

    #[test]
    fn ssrf_denies_v6_6to4() {
        assert!(ssrf_check(v6("2002::1")).is_err());
        assert!(ssrf_check(v6("2002:c0a8:0101::1")).is_err()); // 6to4 wrapping 192.168.1.1
        // boundary: 2001:: is Teredo (next), 2003:: is public
        assert!(ssrf_check(v6("2003::1")).is_ok());
    }

    #[test]
    fn ssrf_denies_v6_teredo() {
        assert!(ssrf_check(v6("2001:0::1")).is_err());
        assert!(ssrf_check(v6("2001:0:abcd::1")).is_err());
        // boundary: 2001:db8:: (documentation) has seg[1] != 0 → NOT Teredo → public
        assert!(ssrf_check(v6("2001:db8::1")).is_ok());
    }

    #[test]
    fn ssrf_denies_v6_ipv4_compatible_embedded() {
        // ::a.b.c.d (first 96 bits zero) embedding a denied v4 → denied.
        assert!(ssrf_check(v6("::a00:1")).is_err()); // ::10.0.0.1
        assert!(ssrf_check(v6("::a9fe:a9fe")).is_err()); // ::169.254.169.254
        assert!(ssrf_check(v6("::7f00:1")).is_err()); // ::127.0.0.1
    }

    // ---- validate_url ----

    #[test]
    fn validate_url_rejects_userinfo() {
        assert!(validate_url("https://user:pass@api.github.com/x").is_err());
        assert!(validate_url("https://user@api.github.com/x").is_err());
    }

    #[test]
    fn validate_url_rejects_non_https() {
        assert!(validate_url("http://api.github.com/x").is_err());
        assert!(validate_url("ftp://api.github.com/x").is_err());
        assert!(validate_url("file:///etc/passwd").is_err());
    }

    #[test]
    fn validate_url_rejects_ip_encoding_tricks() {
        assert!(validate_url("https://127.0.0.1/x").is_err()); // plain v4 literal
        assert!(validate_url("https://2130706433/x").is_err()); // decimal 127.0.0.1
        assert!(validate_url("https://0x7f000001/x").is_err()); // hex 127.0.0.1
        assert!(validate_url("https://0177.0.0.1/x").is_err()); // octal 127.0.0.1
        assert!(validate_url("https://[::1]/x").is_err()); // v6 literal
    }

    #[test]
    fn validate_url_accepts_plain_https_domain() {
        assert_eq!(validate_url("https://api.github.com/x").unwrap(), "api.github.com");
    }

    #[test]
    fn validate_url_rejects_non_default_port() {
        // FIX 2: an explicit non-default port is rejected — the pin only fixes
        // the IP, not the port, so "checked endpoint" must equal "connected
        // endpoint" in the port dimension too.
        assert!(validate_url("https://api.github.com:8080/x").is_err());
        assert!(validate_url("https://api.github.com:22/x").is_err());
        // The default https port (443) is NOT "explicit" per the WHATWG parser
        // (Url::port() returns None), so it is still accepted.
        assert_eq!(validate_url("https://api.github.com:443/x").unwrap(), "api.github.com");
    }

    // ---- host allowlist ----

    #[test]
    fn allowlist_accepts_allowlisted_host() {
        assert!(is_host_allowlisted("api.github.com"));
        assert!(is_host_allowlisted("API.GITHUB.COM")); // case-insensitive
    }

    #[test]
    fn allowlist_rejects_non_allowlisted_host() {
        assert!(!is_host_allowlisted("evil.example.com"));
        assert!(!is_host_allowlisted("api.github.com.evil.com"));
    }

    // ---- mock-egress-ca: the release trust set + allowlist are PROVABLY
    // unchanged with the feature OFF; the feature adds EXACTLY one anchor + one
    // host when ON (T-40-03). These are the LIVE-03 security-invariant tests. ----

    /// RELEASE / default build: the egress host allowlist is EXACTLY
    /// [api.github.com] — the mock host is NOT reachable without the feature.
    /// Gated `not(mock-egress-ca)`: it asserts the feature-OFF invariant, which
    /// by definition does not hold once the feature is compiled in.
    #[cfg(not(feature = "mock-egress-ca"))]
    #[test]
    fn allowlist_default_build_is_api_github_only() {
        assert_eq!(HOST_ALLOWLIST, &["api.github.com"]);
        // The Plan 40-03 mock host is NOT allowlisted in a default build.
        assert!(!is_host_allowlisted("github-mock.caprun.test"));
    }

    /// RELEASE / default build: the egress trust set is EXACTLY the
    /// `webpki-roots` anchors — no extra test CA. This is the core T-40-03
    /// assertion the closing adversarial review relies on. Gated
    /// `not(mock-egress-ca)`: it asserts the feature-OFF invariant.
    #[cfg(not(feature = "mock-egress-ca"))]
    #[test]
    fn egress_root_store_default_build_is_webpki_roots_only() {
        assert_eq!(
            egress_root_store().roots.len(),
            webpki_roots::TLS_SERVER_ROOTS.len(),
            "default build must trust webpki-roots ONLY — no extra anchor"
        );
    }

    /// FEATURE ON (`--features mock-egress-ca`): the mock host is allowlisted
    /// AND api.github.com is still allowlisted; the trust set is webpki-roots
    /// + EXACTLY ONE (the test CA).
    #[cfg(feature = "mock-egress-ca")]
    #[test]
    fn mock_egress_ca_feature_adds_exactly_one_host_and_one_anchor() {
        // The extra host is reachable...
        assert!(is_host_allowlisted("github-mock.caprun.test"));
        assert!(is_host_allowlisted("GITHUB-MOCK.CAPRUN.TEST")); // case-insensitive
        // ...and the shipped host still is.
        assert!(is_host_allowlisted("api.github.com"));
        // Trust set is webpki-roots + exactly one (the checked-in test CA).
        assert_eq!(
            egress_root_store().roots.len(),
            webpki_roots::TLS_SERVER_ROOTS.len() + 1,
            "feature ON must add EXACTLY one trust anchor"
        );
    }

    // ---- vet_resolved: fail-closed SSRF vetting over the resolved set ----

    #[test]
    fn vet_resolved_denies_if_any_ip_denied() {
        let public: SocketAddr = "140.82.112.3:443".parse().unwrap();
        let loopback: SocketAddr = "127.0.0.1:443".parse().unwrap();
        // a mixed answer (one public + one loopback) denies the whole request
        assert!(vet_resolved(&[public, loopback]).is_err());
        assert!(vet_resolved(&[loopback]).is_err());
    }

    #[test]
    fn vet_resolved_denies_empty() {
        assert!(vet_resolved(&[]).is_err());
    }

    #[test]
    fn vet_resolved_pins_first_when_all_public() {
        let a: SocketAddr = "140.82.112.3:443".parse().unwrap();
        let b: SocketAddr = "140.82.113.4:443".parse().unwrap();
        assert_eq!(vet_resolved(&[a, b]).unwrap(), a);
    }

    // ---- response body cap (FIX 3, host-portable) ----

    #[test]
    fn body_cap_is_fail_closed_at_the_boundary() {
        // Exactly at the cap is OK; one byte over is an Err — never a truncate.
        assert!(check_body_cap(MAX_RESPONSE_BODY_BYTES, MAX_RESPONSE_BODY_BYTES).is_ok());
        assert!(check_body_cap(MAX_RESPONSE_BODY_BYTES + 1, MAX_RESPONSE_BODY_BYTES).is_err());
    }

    #[test]
    fn body_cap_trips_while_accumulating_synthetic_chunks() {
        // Simulate the do_pinned_get streaming loop over a synthetic body that
        // exceeds a small cap: the running total must trip the cap mid-stream.
        let cap = 16usize;
        let chunks = [vec![0u8; 8], vec![0u8; 8], vec![0u8; 8]]; // 24 bytes total
        let mut total = 0usize;
        let mut tripped = false;
        for c in &chunks {
            total += c.len();
            if check_body_cap(total, cap).is_err() {
                tripped = true;
                break;
            }
        }
        assert!(tripped, "24 bytes past a 16-byte cap must fail-closed mid-stream");
        assert_eq!(total, 24, "cap should trip after the third chunk pushed total over 16");
    }

    // ---- reqwest client wiring (host-portable, no network) ----

    #[test]
    fn build_pinned_client_constructs_redirect_free_pinned_client() {
        // Exercises the ring provider + webpki-roots TLS config + redirect(none)
        // + .resolve() pin at client-build time on macOS — no socket opened.
        let pinned: SocketAddr = "140.82.112.3:443".parse().unwrap();
        assert!(build_pinned_client("api.github.com", pinned).is_ok());
    }

    // ---- invoke_http_get: allowlist gate precedes any resolve ----

    #[tokio::test]
    async fn invoke_http_get_rejects_non_allowlisted_host_without_network() {
        // A non-allowlisted host must Err at the allowlist gate, BEFORE any DNS
        // resolve/socket. Uses an unresolvable TLD so that if the allowlist gate
        // were (incorrectly) bypassed, a resolve attempt would be observable;
        // the fast Err proves the gate precedes resolve.
        let r = invoke_http_get("https://evil.invalid/x").await;
        assert!(r.is_err());
    }

    #[tokio::test]
    async fn invoke_http_get_rejects_non_https_before_allowlist() {
        // validate_url runs first: a non-https URL Errs even for an allowlisted host.
        assert!(invoke_http_get("http://api.github.com/x").await.is_err());
    }

    // ---- invoke_pinned_post: the github.pr POST egress (Plan 38-03 Task 1) ----
    // Reuses the SAME validate_url → allowlist gate → resolve-and-pin path as the
    // GET; the only additions are method=POST, the bearer + github headers, and a
    // request body. These host-portable tests exercise the pre-socket gates.

    #[tokio::test]
    async fn invoke_pinned_post_rejects_non_allowlisted_host_before_resolve() {
        // A non-allowlisted host must Err at the allowlist gate, BEFORE any DNS
        // resolve/socket (mirror invoke_http_get's gate test). An unresolvable
        // TLD makes a bypassed gate observable as a resolve attempt; the fast Err
        // proves the gate precedes resolve.
        let r = invoke_pinned_post("https://evil.invalid/x", "tok", "{}").await;
        assert!(r.is_err());
    }

    #[tokio::test]
    async fn invoke_pinned_post_rejects_non_https_base() {
        // validate_url runs first: a non-https base Errs even for an allowlisted host.
        assert!(invoke_pinned_post("http://api.github.com/x", "tok", "{}")
            .await
            .is_err());
    }

    #[tokio::test]
    async fn invoke_pinned_post_rejects_userinfo_base() {
        // A userinfo-bearing base is rejected at validate_url (SSRF/credential
        // smuggling) before any resolve — same defense as the GET path.
        assert!(
            invoke_pinned_post("https://user:pass@api.github.com/x", "tok", "{}")
                .await
                .is_err()
        );
    }
}
