//! sinks/http_request — the broker-side, read-only `http.request` GET egress
//! (HTTP-01/HTTP-03, DESIGN §3.1 Pattern A).
//!
//! # RED PHASE (TDD) — stubs only
//!
//! This file is committed first as failing `todo!()` stubs plus the full
//! host-portable test suite (RED), then filled in (GREEN). Do not ship the
//! `todo!()` bodies.
use anyhow::Result;
use std::net::{IpAddr, SocketAddr};

/// The fetch-target host allowlist (DESIGN §3.6 + §11 — an operator-surfaced
/// deployment CONSTANT, never runtime-configurable from a plan node /
/// `ValueNode` / audit DB). This is a security property (SSRF/egress bound),
/// mirroring `email_smtp.rs`'s broker-owned D-04 trusted endpoint config: it is
/// broker-local trusted config, NOT a swappable policy file.
const HOST_ALLOWLIST: &[&str] = &["api.github.com"];

/// True iff `host` is on the hardcoded allowlist (case-insensitive). A
/// non-allowlisted host is rejected by `invoke_http_get` BEFORE any DNS
/// resolve. Pure, host-portable.
fn is_host_allowlisted(_host: &str) -> bool {
    todo!("GREEN")
}

/// Validate a fetch URL and return its DNS hostname. Rejects a `userinfo@`
/// component, any scheme other than `https`, and IP-encoding tricks
/// (decimal/octal/hex-packed or plain IP-literal hosts — the WHATWG URL parser
/// normalizes those to an `Ipv4`/`Ipv6` host, which we reject: only a DNS
/// domain host is allowed). Pure, host-portable.
fn validate_url(_url: &str) -> Result<String> {
    todo!("GREEN")
}

/// The load-bearing SSRF classifier (DESIGN §3.6). Returns `Err` for a resolved
/// IP in any denied range: loopback, RFC1918, link-local (incl. cloud-metadata
/// 169.254.169.254), CGNAT, ULA, IPv6-mapped-IPv4 (embedded v4 re-checked),
/// unspecified. `Ok` for an ordinary public IP. Pure over `IpAddr`,
/// host-portable — the same check that runs on the IP reqwest is then pinned to.
pub fn ssrf_check(_ip: IpAddr) -> Result<()> {
    todo!("GREEN")
}

/// Vet a set of resolved socket addresses and return the one to pin. Fail-closed:
/// if the set is empty, or if ANY resolved IP is in a denied SSRF range, returns
/// `Err` (a mixed good/bad DNS answer denies the whole request). Pure over the
/// resolved list (no DNS) → host-portable and unit-testable.
fn vet_resolved(_addrs: &[SocketAddr]) -> Result<SocketAddr> {
    todo!("GREEN")
}

/// Build the redirect-free, IP-pinned reqwest client for a vetted destination.
/// Host-portable (no network): compiles + type-checks the ring/webpki-roots TLS
/// wiring on macOS. `redirect(Policy::none())` (T-37-04) + `.resolve(host,
/// pinned)` pins the connect target to the SSRF-vetted IP (SNI/Host = original
/// hostname), closing the DNS-rebind TOCTOU (T-37-03).
fn build_pinned_client(_host: &str, _pinned: SocketAddr) -> Result<reqwest::Client> {
    todo!("GREEN")
}

/// Broker-side read-only GET egress. Order: validate_url → allowlist gate
/// (Err BEFORE any resolve) → [Linux] resolve → vet all resolved IPs
/// (`ssrf_check`) → pin to the vetted IP → GET with redirects OFF → body text.
///
/// This function performs NO minting, appends NO audit event, and does NOT
/// touch session status — Plan 03 owns all of that (keeps this module out of
/// Gate 3's mint-site restriction, DESIGN §10).
pub async fn invoke_http_get(_url: &str) -> Result<String> {
    todo!("GREEN")
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
