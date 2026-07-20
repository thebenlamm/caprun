#!/usr/bin/env bash
# docker-cache.sh — retention policy + prune tool for this repo's OWN Docker
# volumes (2026-07-20 incident response).
#
# SCOPE GUARDRAIL — read before editing: this script touches ONLY Docker
# volumes named `caprun-*`. It NEVER touches Colima/Lima VM-level state
# (VM disk size, VM recreation, `colima start`/`stop` flags, `colima
# delete`, `~/.colima` config). The Colima VM is SHARED infrastructure used
# by other projects on this machine — any VM-level disk growth is handled
# separately, at the machine level, not by this repo. If you find yourself
# adding a `colima` invocation here, stop — that belongs somewhere else.
#
# Background: two ad hoc named volumes, `caprun-lt` and `caprun-lt-cache`,
# were manually bound as CARGO_TARGET_DIR mounts (e.g. `-v caprun-lt:/tmp/lt`)
# during a single autonomous v1.9 milestone session on 2026-07-18 to speed up
# repeated `rust:1` compiles, and never cleaned up afterward. They silently
# grew to ~16GB combined (two near-duplicate target/debug trees) with zero
# containers ever referencing them again by the time anyone looked. Deleted
# 2026-07-20. Neither checked-in verification script (mailpit-verify.sh,
# compose-verify.sh) creates a named volume today — both always use an
# EPHEMERAL CARGO_TARGET_DIR inside a `--rm` container, so nothing in this
# repo currently drives this growth. This script exists so that IF that
# pattern recurs (a developer or an agent manually adding a named-volume
# cache for iteration speed), it stays bounded and visible instead of
# silently reaching double digits of GB again. See README.md "Docker Cache
# Policy" for the full policy this enforces.
#
# Policy:
#   - Any persistent build-cache volume for Linux verification MUST be
#     named `caprun-<something>` (so this tool and the guard below can find
#     it) and there should be exactly ONE such volume, not several — two
#     near-identical caches is what actually caused the 16GB incident, not
#     the existence of a cache per se.
#   - Warn threshold: DOCKER_CACHE_WARN_GB (default 8) total GB across all
#     `caprun-*` volumes. A full workspace `target/debug` is legitimately
#     several GB, so this is a circuit breaker against DUPLICATION/runaway
#     growth, not a claim that any single-digit-GB cache is itself a bug.
#   - `check` (below) is wired into mailpit-verify.sh and compose-verify.sh
#     — it runs on every Linux verification call, the closest thing this
#     project has to a standing gate (no CI, no pre-commit framework exist
#     in this repo yet). It only WARNS; it never fails the verification run.
#
# Usage:
#   scripts/docker-cache.sh check           # warn-only; silent if clean/under cap
#   scripts/docker-cache.sh status          # always print current caprun-* volumes
#   scripts/docker-cache.sh clean           # prune caprun-* volumes (prompts first)
#   scripts/docker-cache.sh clean --yes     # prune without prompting (agent/CI use)
#
# Env overrides:
#   DOCKER_CACHE_WARN_GB — size cap in GB before `check`/`status` warns (default: 8)

set -euo pipefail

WARN_GB="${DOCKER_CACHE_WARN_GB:-8}"
CMD="${1:-status}"

# Never let this tool break a caller that doesn't have Docker up yet —
# `check` in particular runs unconditionally at the top of the verify
# scripts, before those scripts are guaranteed docker is reachable in every
# code path a future edit might introduce.
if ! command -v docker >/dev/null 2>&1 || ! docker system df -v >/tmp/.docker-cache-df.$$ 2>/dev/null; then
    [ "${CMD}" = "check" ] && exit 0
    echo "docker-cache.sh: docker not reachable — nothing to report." >&2
    exit 0
fi

# Parse the "Local Volumes" table out of `docker system df -v`. Keyed off
# the "VOLUME NAME" column header (unique to this section) rather than the
# preceding "Local Volumes space usage:" line, because a blank separator
# line sits between that heading and the table itself.
volumes=$(awk '
    /^VOLUME NAME/ { infound=1; next }
    infound && NF==0 { infound=0 }
    infound { print $1, $2, $3 }
' /tmp/.docker-cache-df.$$ | awk '$1 ~ /^caprun-/')
rm -f /tmp/.docker-cache-df.$$

to_bytes() {
    local val="$1" num unit
    num=$(echo "${val}" | sed -E 's/[A-Za-z]+$//')
    unit=$(echo "${val}" | sed -E 's/^[0-9.]+//')
    case "${unit}" in
        B|"") awk -v n="${num}" 'BEGIN{printf "%.0f", n}' ;;
        kB) awk -v n="${num}" 'BEGIN{printf "%.0f", n*1000}' ;;
        MB) awk -v n="${num}" 'BEGIN{printf "%.0f", n*1000000}' ;;
        GB) awk -v n="${num}" 'BEGIN{printf "%.0f", n*1000000000}' ;;
        TB) awk -v n="${num}" 'BEGIN{printf "%.0f", n*1000000000000}' ;;
        *) echo 0 ;;
    esac
}

count=0
total_bytes=0
table=""
while IFS=' ' read -r name links size; do
    [ -z "${name}" ] && continue
    count=$((count + 1))
    b=$(to_bytes "${size}")
    total_bytes=$((total_bytes + b))
    table="${table}${name}  links=${links}  size=${size}\n"
done <<<"${volumes}"

total_gb=$(awk -v b="${total_bytes}" 'BEGIN{printf "%.2f", b/1000000000}')

case "${CMD}" in
    check)
        if [ "${count}" -eq 0 ]; then
            exit 0
        fi
        over_cap=$(awk -v t="${total_gb}" -v c="${WARN_GB}" 'BEGIN{print (t>c)?1:0}')
        if [ "${over_cap}" -eq 1 ] || [ "${count}" -gt 1 ]; then
            echo "─────────────────────────────────────────────────────────────" >&2
            echo "docker-cache.sh WARNING: caprun-* Docker volumes need attention" >&2
            printf "%b" "${table}" >&2
            echo "Total: ${total_gb}GB (cap: ${WARN_GB}GB), volume count: ${count} (policy: 1)" >&2
            echo "Run: scripts/docker-cache.sh clean    (see README 'Docker Cache Policy')" >&2
            echo "This is a warning only — the verification run continues." >&2
            echo "─────────────────────────────────────────────────────────────" >&2
        fi
        exit 0
        ;;
    status)
        if [ "${count}" -eq 0 ]; then
            echo "No caprun-* Docker volumes present."
            exit 0
        fi
        printf "%b" "${table}"
        echo "Total: ${total_gb}GB (cap: ${WARN_GB}GB), volume count: ${count} (policy: 1)"
        exit 0
        ;;
    clean)
        if [ "${count}" -eq 0 ]; then
            echo "No caprun-* Docker volumes present — nothing to clean."
            exit 0
        fi
        printf "%b" "${table}"
        echo "Total: ${total_gb}GB across ${count} volume(s)."
        if [ "${2:-}" != "--yes" ]; then
            read -r -p "Remove these caprun-* volumes? [y/N] " reply
            case "${reply}" in
                y|Y|yes|YES) ;;
                *) echo "Aborted — nothing removed."; exit 0 ;;
            esac
        fi
        while IFS=' ' read -r name links size; do
            [ -z "${name}" ] && continue
            echo "Removing ${name} ..."
            docker volume rm "${name}"
        done <<<"${volumes}"
        echo "Done."
        exit 0
        ;;
    *)
        echo "Usage: scripts/docker-cache.sh {check|status|clean [--yes]}" >&2
        exit 1
        ;;
esac
