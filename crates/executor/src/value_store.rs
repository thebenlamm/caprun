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
    pub fn mint(
        &mut self,
        _literal: String,
        _taint: Vec<TaintLabel>,
        _provenance_chain: Vec<uuid::Uuid>,
    ) -> ValueId {
        unimplemented!("ValueStore::mint — RED phase stub")
    }

    /// Read-only dereference of an opaque handle to its authoritative ValueRecord.
    ///
    /// Returns `None` for any id not previously minted by this store. The executor's
    /// decision rule treats `None` as `Denied` — a dangling/forged handle MUST NOT
    /// become `Allowed`.
    pub fn resolve(&self, _id: &ValueId) -> Option<&ValueRecord> {
        unimplemented!("ValueStore::resolve — RED phase stub")
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

        let id = store.mint(literal.clone(), taint.clone(), chain.clone());
        let record = store.resolve(&id).expect("minted id must resolve");

        assert_eq!(record.id, id, "resolved record id must match minted ValueId");
        assert_eq!(record.literal, literal);
        assert_eq!(record.taint, taint);
        assert_eq!(
            record.provenance_chain, chain,
            "provenance_chain[0] must equal the file_read Event id"
        );
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
}
