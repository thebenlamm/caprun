#!/usr/bin/env bash
# install-linux.sh — minimal Linux source-build installer for caprun (Phase 52, PKG-01)
#
# Builds the release targets and co-locates the three required sibling
# binaries — caprun, caprun-worker, caprun-exec-launcher — in ONE destination
# directory. Co-location is a functional deployment invariant, not a
# documentation preference: caprun's worker lookup
# (cli/caprun/src/main.rs:607-611) is a SINGLE current_exe().parent() hop
# with no fallback search, so caprun-worker must be a direct sibling of
# caprun or the worker spawn fails with a bare OS "No such file or
# directory". caprun-exec-launcher's resolution
# (crates/brokerd/src/sinks/process_exec.rs:736-756) additionally tolerates
# up to 2 ancestor directories (a cargo-test accommodation only, not an
# install-layout feature), but is satisfied — and simplest — by direct
# co-location too.
#
# Steps:
#   1. Resolve the destination (--dest flag > INSTALL_DEST env >
#      ${HOME}/.local/bin).
#   2. cargo build --workspace --release.
#   3. Assert the three release binaries exist.
#   4. Stage all three into a temp directory INSIDE the destination
#      (same-filesystem renames), verify each is executable.
#   5. Move the three staged binaries into the destination back to back, in
#      a fixed order (helpers first, orchestrator last).
#   6. Post-install layout check: all three paths exist and are executable.
#   7. Print what was installed, plus a PATH hint if the destination isn't
#      already discoverable.
#
# caprun-planner is also produced by the workspace release build but is NOT
# installed — it is optional LLM-sidecar functionality, outside the required
# minimal deterministic set (D-07).
#
# Interruption / concurrency guarantee (BOUNDED, not total — read before
# relying on this for a concurrent or interrupted install): nothing enters
# the destination until all three binaries are staged and proven executable,
# so a failed BUILD or a failed COPY never leaves a partial set behind. The
# three final renames into the destination are atomic INDIVIDUALLY (POSIX
# rename() on the same filesystem) but NOT as a set, and two installs run at
# the same time into the SAME destination are not serialized against each
# other. If a process is killed mid-sequence, or two concurrent installs
# race, the destination can end up with a mixed set from two different
# builds. Recovery: re-run the installer — it is idempotent and will
# overwrite the mixed set with one consistent build's binaries.
#
# Usage:
#   bash scripts/install-linux.sh [--dest <dir>]
#   bash scripts/install-linux.sh --help
# Run from the workspace root (same directory as Cargo.toml).
#
# Env overrides:
#   INSTALL_DEST — destination directory (default: ${HOME}/.local/bin);
#                  overridden by --dest when both are given.

set -euo pipefail

usage() {
    echo "Usage: bash scripts/install-linux.sh [--dest <dir>]" >&2
}

# ── Resolve repo root; require Cargo.toml there ─────────────────────────────
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [ ! -f "${REPO_ROOT}/Cargo.toml" ]; then
    echo "FAIL — could not find Cargo.toml at ${REPO_ROOT}; run this script from a caprun checkout (Run from the workspace root)" >&2
    exit 1
fi
cd "${REPO_ROOT}"

# ── Argument dispatch ────────────────────────────────────────────────────────
DEST_FLAG=""
while [ $# -gt 0 ]; do
    case "$1" in
        --dest)
            if [ $# -lt 2 ]; then
                echo "FAIL — --dest requires a directory argument" >&2
                usage
                exit 1
            fi
            DEST_FLAG="$2"
            shift 2
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        *)
            echo "FAIL — unrecognized argument: $1" >&2
            usage
            exit 1
            ;;
    esac
done

# ── Destination resolution (flag > env > default; D-04/D-05) ────────────────
DEST="${DEST_FLAG:-${INSTALL_DEST:-$HOME/.local/bin}}"

# ── Build (D-02) ──────────────────────────────────────────────────────────────
echo "Building release binaries (cargo build --workspace --release) ..."
if ! cargo build --workspace --release; then
    echo "FAIL — cargo build --workspace --release failed; see the compiler output above" >&2
    exit 1
fi

# ── Assert the three release binaries exist ──────────────────────────────────
for bin in caprun caprun-worker caprun-exec-launcher; do
    if [ ! -f "target/release/${bin}" ]; then
        echo "FAIL — target/release/${bin} missing after the release build" >&2
        exit 1
    fi
done

# ── Stage (D-06: same-filesystem renames, no visibly mixed set on failure) ───
mkdir -p "${DEST}"
STAGE_DIR="$(mktemp -d "${DEST}/.caprun-install.XXXXXX")"
cleanup() {
    rm -rf "${STAGE_DIR}"
}
trap cleanup EXIT

for bin in caprun caprun-worker caprun-exec-launcher; do
    install -m 755 "target/release/${bin}" "${STAGE_DIR}/${bin}"
    if [ ! -x "${STAGE_DIR}/${bin}" ]; then
        echo "FAIL — ${STAGE_DIR}/${bin} was staged but is not executable" >&2
        exit 1
    fi
done

# ── Install: move staged binaries into the destination back to back, helpers
#    first, orchestrator last — a kill mid-sequence leaves an already-present
#    orchestrator paired with fresh helpers rather than the reverse.
mv -f "${STAGE_DIR}/caprun-worker" "${DEST}/caprun-worker"
mv -f "${STAGE_DIR}/caprun-exec-launcher" "${DEST}/caprun-exec-launcher"
mv -f "${STAGE_DIR}/caprun" "${DEST}/caprun"

# ── Post-install layout check (D-09, non-destructive) ─────────────────────────
for bin in caprun caprun-worker caprun-exec-launcher; do
    if [ ! -x "${DEST}/${bin}" ]; then
        echo "FAIL — ${DEST}/${bin} missing or not executable" >&2
        exit 1
    fi
done

echo "${DEST}/caprun"
echo "${DEST}/caprun-worker"
echo "${DEST}/caprun-exec-launcher"
echo "PASS — all 3 required binaries installed and executable in ${DEST}"

# ── PATH hint (D-05) ──────────────────────────────────────────────────────────
case ":$PATH:" in
    *":${DEST}:"*) ;;
    *) echo "NOTE — ${DEST} is not on your PATH. Add it, e.g.: export PATH=\"${DEST}:\$PATH\"" ;;
esac
