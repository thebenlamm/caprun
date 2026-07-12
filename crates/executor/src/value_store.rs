/// value_store.rs — ValueStore: the broker-owned in-memory value store.
///
/// `ValueStore::mint` is the SOLE taint writer in the executor crate.
/// The executor's decision function (`submit_plan_node`) reads records only
/// through `resolve` — it NEVER writes and NEVER constructs a ValueRecord.
/// This is the anti-stapling invariant (T-04-03, T-04-01).
///
/// Corresponds to DESIGN-plan-executor §ValueRecord & ValueId Handle Model:
/// the broker/worker-extraction step calls mint; the planner never calls mint;
/// the executor calls only resolve.

use std::collections::HashMap;

// Import from runtime-core — NOT redefined here.
// ValueId and ValueRecord are broker-defined types; redefining them here would
// break the single-authority invariant (the type system would allow
// executor-forged records).
use runtime_core::plan_node::{TaintLabel, ValueId};
use runtime_core::value_record::ValueRecord;

/// In-memory broker-owned value store: maps opaque `ValueId` handles to their
/// authoritative `ValueRecord` (literal + taint + provenance_chain).
///
/// `ValueStore::mint` is the only path in this crate that writes a record's
/// taint field. `ValueStore::resolve` is read-only. The executor's decision
/// function uses only `resolve`.
#[derive(Debug, Default)]
pub struct ValueStore {
    inner: HashMap<ValueId, ValueRecord>,
}

/// Error returned by `ValueStore::mint` when the mint-time non-empty invariant is
/// violated. Making `mint` fallible means an empty-taint or empty-provenance
/// `ValueRecord` is UNCONSTRUCTABLE through the sanctioned path (HARD-05) — the
/// invariant holds by construction, not merely by convention.
#[derive(Debug, Clone, PartialEq)]
pub enum MintInvariantError {
    /// `taint` was empty. Every ValueRecord must carry ≥1 taint label; an empty
    /// taint would skip the executor's `any(is_untrusted)` block (an empty
    /// iterator is never untrusted) and fail open past the sensitivity check.
    EmptyTaint,
    /// `provenance_chain` was empty. `provenance_chain[0]` is the genuine-taint
    /// anchor (the originating read/intent Event id); an empty chain breaks it.
    EmptyProvenance,
}

impl ValueStore {
    /// Mint a new `ValueRecord` and return its opaque `ValueId` handle.
    ///
    /// This is the SOLE taint writer in the executor crate. It MUST be called by
    /// the broker's worker-extraction path (when the quarantined worker returns a
    /// typed extract from a read Event). The planner NEVER calls mint — it holds
    /// only the returned `ValueId`.
    ///
    /// `provenance_chain[0]` MUST equal the originating file_read Event id; this
    /// is the field the §9 held-out test asserts to prove genuine taint propagation.
    ///
    /// Fails closed (HARD-05): rejects empty taint (`EmptyTaint`) and empty
    /// provenance (`EmptyProvenance`) BEFORE minting an id, so no empty-taint or
    /// empty-provenance record can ever enter the store.
    pub fn mint(
        &mut self,
        literal: String,
        taint: Vec<TaintLabel>,
        provenance_chain: Vec<uuid::Uuid>,
        origin_role: Option<String>,
    ) -> Result<ValueId, MintInvariantError> {
        if taint.is_empty() {
            return Err(MintInvariantError::EmptyTaint);
        }
        if provenance_chain.is_empty() {
            return Err(MintInvariantError::EmptyProvenance);
        }
        let id = ValueId::new();
        let record = ValueRecord {
            id: id.clone(),
            literal,
            taint,
            provenance_chain,
            origin_role,
        };
        self.inner.insert(id.clone(), record);
        Ok(id)
    }

    /// Read-only dereference of an opaque handle to its authoritative ValueRecord.
    ///
    /// Returns `None` for any id not previously minted by this store. The executor's
    /// decision rule treats `None` as `Denied` — a dangling/forged handle MUST NOT
    /// become `Allowed`.
    pub fn resolve(&self, id: &ValueId) -> Option<&ValueRecord> {
        self.inner.get(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use runtime_core::plan_node::{TaintLabel, ValueId};
    use uuid::Uuid;

    /// mint-then-resolve round-trip: the resolved record's literal, taint,
    /// and provenance_chain must match exactly what was passed to mint.
    #[test]
    fn mint_then_resolve_round_trip() {
        let mut store = ValueStore::default();
        let literal = "accounts@ev1l.com".to_string();
        let taint = vec![TaintLabel::ExternalUntrusted, TaintLabel::EmailRaw];
        let event_id = Uuid::new_v4();
        let chain = vec![event_id];

        let id = store
            .mint(literal.clone(), taint.clone(), chain.clone(), None)
            .expect("valid mint");
        let record = store.resolve(&id).expect("minted id must resolve");

        assert_eq!(record.id, id, "resolved record id must match minted ValueId");
        assert_eq!(record.literal, literal);
        assert_eq!(record.taint, taint);
        assert_eq!(
            record.provenance_chain, chain,
            "provenance_chain[0] must equal the file_read Event id"
        );
    }

    /// origin_role is threaded verbatim onto the record: mint(..., Some(role))
    /// then resolve() returns the same role unchanged (T2, DESIGN-slot-type-binding.md §1).
    #[test]
    fn mint_threads_origin_role_verbatim() {
        let mut store = ValueStore::default();
        let event_id = Uuid::new_v4();
        let id = store
            .mint(
                "user@example.com".to_string(),
                vec![TaintLabel::UserTrusted],
                vec![event_id],
                Some("recipient".to_string()),
            )
            .expect("valid mint");
        let record = store.resolve(&id).expect("minted id must resolve");
        assert_eq!(record.origin_role, Some("recipient".to_string()));
    }

    /// origin_role is optional: mint(..., None) round-trips to None on resolve.
    #[test]
    fn mint_with_no_origin_role_resolves_to_none() {
        let mut store = ValueStore::default();
        let event_id = Uuid::new_v4();
        let id = store
            .mint(
                "doc fragment text".to_string(),
                vec![TaintLabel::WorkerExtracted],
                vec![event_id],
                None,
            )
            .expect("valid mint");
        let record = store.resolve(&id).expect("minted id must resolve");
        assert_eq!(record.origin_role, None);
    }

    /// resolve of a random ValueId returns None — no forgery possible.
    #[test]
    fn resolve_unknown_id_returns_none() {
        let store = ValueStore::default();
        let random_id = ValueId::new();
        assert!(
            store.resolve(&random_id).is_none(),
            "unknown ValueId must resolve to None"
        );
    }

    // -----------------------------------------------------------------------
    // Mint non-empty invariant (HARD-05): mint MUST reject empty taint or empty
    // provenance so an empty-taint/empty-provenance ValueRecord is unconstructable
    // through the sanctioned path. See planning-docs/TASK-mint-nonempty-invariant.md.
    // -----------------------------------------------------------------------

    /// Empty taint is rejected: `mint` returns `Err(MintInvariantError::EmptyTaint)`.
    #[test]
    fn mint_rejects_empty_taint() {
        let mut store = ValueStore::default();
        let event_id = Uuid::new_v4();
        let result = store.mint("boss@company.com".to_string(), vec![], vec![event_id], None);
        assert_eq!(
            result,
            Err(MintInvariantError::EmptyTaint),
            "empty taint must be rejected — an empty-taint record is unconstructable"
        );
    }

    /// Empty provenance is rejected: `mint` returns `Err(MintInvariantError::EmptyProvenance)`.
    #[test]
    fn mint_rejects_empty_provenance() {
        let mut store = ValueStore::default();
        let result = store.mint(
            "boss@company.com".to_string(),
            vec![TaintLabel::UserTrusted],
            vec![],
            None,
        );
        assert_eq!(
            result,
            Err(MintInvariantError::EmptyProvenance),
            "empty provenance must be rejected — the taint anchor would be dangling"
        );
    }

    /// Non-empty taint AND provenance succeeds: `mint` returns `Ok(id)` and the
    /// resolved record's taint/provenance_chain match the mint inputs.
    #[test]
    fn mint_accepts_nonempty_taint_and_provenance() {
        let mut store = ValueStore::default();
        let event_id = Uuid::new_v4();
        let taint = vec![TaintLabel::UserTrusted];
        let chain = vec![event_id];
        let id = store
            .mint(
                "boss@company.com".to_string(),
                taint.clone(),
                chain.clone(),
                None,
            )
            .expect("non-empty taint + provenance must mint Ok");
        let record = store.resolve(&id).expect("minted id must resolve");
        assert_eq!(record.taint, taint);
        assert_eq!(record.provenance_chain, chain);
    }
}
