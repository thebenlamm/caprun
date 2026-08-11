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
#   1. Verify the host is Linux and cargo is on PATH (fail fast, actionable).
#   2. Resolve the destination (--dest flag > INSTALL_DEST env >
#      ${HOME}/.local/bin) and verify it is writable before anything else
#      touches it.
#   3. cargo build --workspace --release.
#   4. Assert the three release binaries exist.
#   5. Stage all three into a temp directory INSIDE the destination
#      (same-filesystem renames), verify each is executable.
#   6. Move the three staged binaries into the destination back to back, in
#      a fixed order (helpers first, orchestrator last).
#   7. Post-install layout check: all three paths exist and are executable.
#   8. Print what was installed, a PATH hint if the destination isn't
#      already discoverable, and an advisory (non-blocking) kernel-version
#      note if Landlock's full confinement guarantee needs a newer kernel.
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
            if [ $# -lt 2 ] || [ -z "$2" ]; then
                echo "FAIL — --dest requires a non-empty directory argument" >&2
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

# ── Operating-system guard (D-03) ────────────────────────────────────────────
OS_NAME="$(uname -s)"
if [ "${OS_NAME}" != "Linux" ]; then
    echo "FAIL — detected ${OS_NAME}; scripts/install-linux.sh only supports Linux (see docs/GETTING-STARTED.md's platform table for other platforms)" >&2
    exit 1
fi

# ── Toolchain guard (D-03) ────────────────────────────────────────────────────
if ! command -v cargo >/dev/null 2>&1; then
    echo "FAIL — cargo not found on PATH; install it via rustup (https://rustup.rs) and re-run" >&2
    exit 1
fi

# ── Destination resolution (flag > env > default; D-04/D-05) ────────────────
DEST="${DEST_FLAG:-${INSTALL_DEST:-$HOME/.local/bin}}"

# ── Destination usability guard (D-03/D-06) — fires BEFORE the build, the
#    staging directory, or any copy touches the destination, so a refused
#    re-install never partially replaces a previously installed set.
mkdir -p "${DEST}" 2>/dev/null || true
if [ -e "${DEST}" ] && [ ! -d "${DEST}" ]; then
    echo "FAIL — ${DEST} exists and is not a directory; choose a different --dest" >&2
    exit 1
fi
if [ ! -d "${DEST}" ] || ! test -w "${DEST}"; then
    echo "FAIL — ${DEST} is not writable; choose a writable destination with --dest, or create/chown it yourself. This script never escalates privileges to obtain one." >&2
    exit 1
fi

# Pre-flight collision guard: fail closed BEFORE the build/stage work if one
# of the three required names already exists as a directory at the
# destination. `mv -f` onto an existing directory target does not overwrite
# it — it nests the staged file inside, silently, which would otherwise
# break the sibling co-location caprun's worker lookup depends on
# (cli/caprun/src/main.rs:607-611).
for bin in caprun caprun-worker caprun-exec-launcher; do
    if [ -d "${DEST}/${bin}" ]; then
        echo "FAIL — ${DEST}/${bin} already exists as a directory; remove it or choose a different --dest" >&2
        exit 1
    fi
done

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
# ${DEST} already exists and is writable — verified by the guard above.
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
# Must assert regular-file-ness, not just `-x`: `test -x` is also true for a
# directory the invoking user can traverse, so a pre-existing same-named
# directory at the destination (see the pre-flight guard above) could
# otherwise report PASS despite the binary never actually landing there.
for bin in caprun caprun-worker caprun-exec-launcher; do
    if [ ! -f "${DEST}/${bin}" ] || [ ! -x "${DEST}/${bin}" ]; then
        echo "FAIL — ${DEST}/${bin} missing, not a regular file, or not executable" >&2
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

# ── Kernel advisory (warn only, never blocks — D-03) ─────────────────────────
# crates/sandbox already negotiates the Landlock ABI down at runtime, so this
# install still succeeds and still exits 0 on an older kernel. A plain shell
# numeric split of the first two dot-separated fields of `uname -r` — not
# general semver parsing, matching docs/GETTING-STARTED.md's existing
# "Common setup issues" wording (`uname -r`).
KERNEL_RELEASE="$(uname -r)"
KERNEL_MAJOR="$(echo "${KERNEL_RELEASE}" | cut -d. -f1)"
KERNEL_MINOR="$(echo "${KERNEL_RELEASE}" | cut -d. -f2 | grep -oE '^[0-9]+' || echo 0)"
if [ "${KERNEL_MAJOR}" -lt 5 ] || { [ "${KERNEL_MAJOR}" -eq 5 ] && [ "${KERNEL_MINOR}" -lt 13 ]; }; then
    echo "NOTE — detected kernel ${KERNEL_RELEASE}; Landlock needs Linux 5.13 or newer for the full confinement guarantee. caprun will still run — crates/sandbox negotiates the Landlock ABI down at runtime."
fi
