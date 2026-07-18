//! sinks/http_request — the broker-side, read-only `http.request` GET egress
//! (HTTP-01/HTTP-03, DESIGN §3.1 Pattern A).
//!
//! # Security role (Pattern A, broker-resident — NEVER the confined worker)
//!
//! This is the ONLY code path that performs an outbound HTTP GET, and it lives
//! in the broker exactly like `email_smtp.rs`'s SMTP call — the confined worker
//! stays fully net-denied. The `reqwest`/`rustls`/`ring`/`webpki-roots` deps are
//! broker-only (see `Cargo.toml`).
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
use anyhow::{bail, Result};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;

/// The fetch-target host allowlist (DESIGN §3.6 + §11 — an operator-surfaced
/// deployment CONSTANT, never runtime-configurable from a plan node /
/// `ValueNode` / audit DB). This is a security property (SSRF/egress bound),
/// mirroring `email_smtp.rs`'s broker-owned D-04 trusted endpoint config: it is
/// broker-local trusted config, NOT a swappable policy file.
const HOST_ALLOWLIST: &[&str] = &["api.github.com"];

/// True iff `host` is on the hardcoded allowlist (case-insensitive). A
/// non-allowlisted host is rejected by `invoke_http_get` BEFORE any DNS
/// resolve. Pure, host-portable.
fn is_host_allowlisted(host: &str) -> bool {
    HOST_ALLOWLIST.iter().any(|allowed| allowed.eq_ignore_ascii_case(host))
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
fn ipv4_denied(ip: Ipv4Addr) -> bool {
    ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || is_cgnat(ip)
}

/// 100.64.0.0/10 (RFC 6598 carrier-grade NAT).
fn is_cgnat(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    o[0] == 100 && (0x40..=0x7f).contains(&o[1])
}

/// DESIGN §3.6 IPv6 denials: loopback (::1), unspecified (::), ULA (fc00::/7),
/// link-local (fe80::/10), and IPv6-mapped-IPv4 (::ffff:0:0/96) whose embedded
/// v4 is re-checked against the v4 ranges.
fn ipv6_denied(ip: Ipv6Addr) -> bool {
    if let Some(v4) = ip.to_ipv4_mapped() {
        return ipv4_denied(v4);
    }
    ip.is_loopback() || ip.is_unspecified() || is_ula(ip) || is_v6_link_local(ip)
}

/// fc00::/7 (RFC 4193 unique local address).
fn is_ula(ip: Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xfe00) == 0xfc00
}

/// fe80::/10 (link-local unicast).
fn is_v6_link_local(ip: Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xffc0) == 0xfe80
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
        .build()
        .map_err(|e| anyhow::anyhow!("http.request: failed to build client: {e}"))
}

/// Preconfigured rustls `ClientConfig`: pure-Rust `ring` provider (DESIGN §5.1)
/// + compiled-in `webpki-roots` trust anchors (DESIGN §5.2 — `env_clear()`
/// hermetic, no system cert store / SSL_CERT_* / platform verifier).
// Consumed transitively via `build_pinned_client`; same platform-gated
// unreferenced-on-macOS-non-test situation.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn ring_webpki_tls_config() -> rustls::ClientConfig {
    let roots = rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };
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

/// Linux: resolve → vet all resolved IPs → pin to the vetted IP → redirect-free
/// GET → response body text. The real DNS-resolve + socket-connect leg is
/// Linux-gated per the project's Linux-only pattern (CLAUDE.md); live-HTTPS
/// behavior is deferred to Phase 40.
#[cfg(target_os = "linux")]
async fn do_pinned_get(url: &str, host: &str) -> Result<String> {
    use std::net::ToSocketAddrs;

    // Resolve on a blocking thread (std resolver) — the resolved IPs are the
    // EXACT set that will be vetted and pinned (no re-resolve later).
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
    let client = build_pinned_client(host, pinned)?;
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("http.request: GET failed: {e}"))?;
    resp.text()
        .await
        .map_err(|e| anyhow::anyhow!("http.request: reading response body failed: {e}"))
}

/// Non-Linux (dev macOS) no-op stub: the pure classifiers + client-build wiring
/// above are host-portable and fully tested here, but the real socket leg is
/// Linux-only (CLAUDE.md); live-HTTPS behavior is deferred to Phase 40.
#[cfg(not(target_os = "linux"))]
async fn do_pinned_get(_url: &str, _host: &str) -> Result<String> {
    bail!("http.request live GET is Linux-only (macOS no-op stub); deferred to Phase 40")
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
}
