//! provenance_proof — the promoted Phase-15 genuine-taint proof predicates.
//!
//! `assert_unbroken_edge`, `genuine_derivation_binds`, and their helper
//! `union_provenance_chains` were originally defined inside the private
//! brokerd test binary `crates/brokerd/tests/extract_provenance_threading.rs`
//! (the phase's HARD-GATE proof, EXTRACT-02/03). Cross-crate test modules are
//! not importable, so a live composed test living in a DIFFERENT crate
//! (`cli/caprun/tests/`) could not reach these checks without reimplementing
//! them — a reimplementation is explicitly forbidden (17-RESEARCH.md "Don't
//! Hand-Roll": reimplementing risks silently weakening the exact check the
//! milestone depends on). This module is the promotion: ONE public source of
//! truth, moved (not copied) verbatim byte-for-byte, consumed by BOTH the
//! existing DB-alone Phase-15 proof and Phase 17's new live proof so any
//! drift fails immediately (COORD-T2).
//!
//! These are read-only audit-VERIFICATION utilities (no mint, no I/O beyond
//! the read connection they are handed) — consistent with brokerd's
//! reference-monitor role and with the already-public `verify_chain` in
//! `audit.rs`.

use crate::audit::find_event_by_id;
use runtime_core::{plan_node::ValueId, Event};
use uuid::Uuid;

/// The order-stable, deduplicated union of several provenance chains --
/// mirrors EXACTLY the same operation `mint_from_derivation` performs when
/// it builds a derived value's own `provenance_chain` from its inputs'
/// chains. Used here to recompute `∪ev.input_provenance_chains` from a
/// `derivation` event's persisted payload, for comparison against an
/// anchor's `provenance_chain` (finding #2's exact vector equality).
pub fn union_provenance_chains(chains: &[Vec<Uuid>]) -> Vec<Uuid> {
    let mut union: Vec<Uuid> = Vec::new();
    for chain in chains {
        for id in chain {
            if !union.contains(id) {
                union.push(*id);
            }
        }
    }
    union
}

/// The reusable EXTRACT-02 per-anchor unbroken-edge assertion.
///
/// Resolves EVERY `provenance_chain` element via `find_event_by_id` (never
/// `find_event_by_type`, whose `LIMIT 1` would silently resolve the WRONG
/// event once >1 event of a type exists per session -- 15-RESEARCH.md
/// Pitfall 3). Requires each resolved event's `event_type == "file_read"`
/// (finding #10: a `derivation` event appearing as a `provenance_chain`
/// element is a fail-closed error -- it is NEVER walked recursively as a
/// chain element, the locked two-graphs-never-share-edges decision).
/// Requires each terminal `file_read`'s taint `is_untrusted()`. AND asserts
/// EXACT vector equality `provenance_chain == expected_roots` (finding #12,
/// identity-pinning) -- "is a file_read" is kept ONLY as the per-element
/// type check, never as the terminal criterion: a re-mint via
/// `mint_from_read` produces a REAL untrusted `file_read`, so type-alone
/// would pass a laundered/re-anchored value (negative control B exercises
/// exactly this).
///
/// Returns `Ok(())` iff the walk fully succeeds; `Err(reason)` on ANY
/// failure (missing/unresolvable element, non-file_read element,
/// non-untrusted terminal, or root-vector mismatch) -- used to both
/// `.expect()` the POSITIVE walk and to assert REJECTION for the negative
/// controls.
pub fn assert_unbroken_edge(
    conn: &rusqlite::Connection,
    session_id: &str,
    provenance_chain: &[Uuid],
    expected_roots: &[Uuid],
) -> Result<(), String> {
    if provenance_chain != expected_roots {
        return Err(format!(
            "provenance_chain {provenance_chain:?} != identity-pinned expected roots \
             {expected_roots:?} (finding #12)"
        ));
    }
    for element_id in provenance_chain {
        let event = find_event_by_id(conn, session_id, &element_id.to_string())
            .map_err(|e| format!("DB error resolving provenance_chain element {element_id}: {e}"))?
            .ok_or_else(|| {
                format!(
                    "provenance_chain element {element_id} does not resolve to any real \
                     event in this session -- the edge is UNPROVEN (fabricated root)"
                )
            })?;
        if event.event_type != "file_read" {
            return Err(format!(
                "provenance_chain element {element_id} resolved to event_type `{}`, not \
                 `file_read` (finding #10 -- a derivation event may never appear as a chain \
                 element, and is never walked recursively as one)",
                event.event_type
            ));
        }
        if !event.taint.iter().any(|t| t.is_untrusted()) {
            return Err(format!(
                "provenance_chain element {element_id}'s file_read event does not carry \
                 untrusted taint"
            ));
        }
    }
    Ok(())
}

/// The finding #2 genuine-derivation predicate, DB-alone: scans ALL of the
/// session's `"derivation"` events via a fresh, NO-LIMIT inline SELECT
/// (never `find_event_by_type`, which is `LIMIT 1` and would return only
/// the FIRST derivation event -- multi-arg/multi-derivation extraction can
/// produce several, and the first may not be the one binding THIS anchor's
/// `value_id`, MEDIUM R2) and returns `true` iff ONE of them satisfies:
/// `ev.derived_value_id == value_id` AND
/// `∪ev.input_provenance_chains == expected_provenance_chain` (exact vector
/// equality) -- the payload-bound edge, NOT a vacuous "a derivation event
/// exists" existence check and NOT mere id-membership.
pub fn genuine_derivation_binds(
    conn: &rusqlite::Connection,
    session_id: &str,
    value_id: &ValueId,
    expected_provenance_chain: &[Uuid],
) -> bool {
    let mut stmt = conn
        .prepare("SELECT payload FROM events WHERE session_id = ?1 AND event_type = 'derivation'")
        .expect("prepare ALL-derivation-events scan (no LIMIT -- MEDIUM R2)");
    let payloads: Vec<String> = stmt
        .query_map(rusqlite::params![session_id], |row| row.get(0))
        .expect("query ALL-derivation-events scan")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect derivation event payloads");

    for payload in payloads {
        let ev: Event = serde_json::from_str(&payload).expect("deserialize derivation event");
        if ev.derived_value_id.as_ref() == Some(value_id) {
            let union = union_provenance_chains(&ev.input_provenance_chains);
            if union == expected_provenance_chain {
                return true;
            }
        }
    }
    false
}
