---
schema_version: 1
open_count: 0
waived_count: 0
fixed_count: 3
total_count: 3
last_updated: 2026-08-08T16:46:36.049Z
---

# Broken Windows Ledger

> Cross-phase defect register. `/gsd-ship` blocks while `open_count > 0`.
> Waive with `gsd-tools windows waive <id> "<reason>"` (reason required).
> Mark fixed with `gsd-tools windows fixed <id>`.

| id | phase | kind | file | line | description | status | reason | recorded_at | resolved_at |
|----|-------|------|------|------|-------------|--------|--------|-------------|-------------|
| 1 | 51 | unrun-verify | cli/caprun/tests/live_acceptance_v1_10_cli.rs |  | LIVE-07 compose-verify unavailable because Docker is not installed on executor host | fixed |  | 2026-08-04T03:11:17.796Z | 2026-08-08T16:46:35.624Z |
| 2 | 51 | unrun-verify | cli/caprun/tests/live_acceptance_v1_10_cli.rs |  | LIVE-08 scoped and full-workspace compose verification unavailable because Docker is not installed on executor host | fixed |  | 2026-08-04T03:16:35.305Z | 2026-08-08T16:46:35.843Z |
| 3 | 51 | unrun-verify | cli/caprun/tests/live_acceptance_v1_10_cli.rs |  | Cargo verification deferred because host lacks pkg-config/OpenSSL development metadata | fixed |  | 2026-08-04T13:44:22.173Z | 2026-08-08T16:46:36.049Z |

````json
[
  {
    "id": 1,
    "kind": "unrun-verify",
    "phase": "51",
    "file": "cli/caprun/tests/live_acceptance_v1_10_cli.rs",
    "line": null,
    "description": "LIVE-07 compose-verify unavailable because Docker is not installed on executor host",
    "status": "fixed",
    "reason": "",
    "recorded_at": "2026-08-04T03:11:17.796Z",
    "resolved_at": "2026-08-08T16:46:35.624Z"
  },
  {
    "id": 2,
    "kind": "unrun-verify",
    "phase": "51",
    "file": "cli/caprun/tests/live_acceptance_v1_10_cli.rs",
    "line": null,
    "description": "LIVE-08 scoped and full-workspace compose verification unavailable because Docker is not installed on executor host",
    "status": "fixed",
    "reason": "",
    "recorded_at": "2026-08-04T03:16:35.305Z",
    "resolved_at": "2026-08-08T16:46:35.843Z"
  },
  {
    "id": 3,
    "kind": "unrun-verify",
    "phase": "51",
    "file": "cli/caprun/tests/live_acceptance_v1_10_cli.rs",
    "line": null,
    "description": "Cargo verification deferred because host lacks pkg-config/OpenSSL development metadata",
    "status": "fixed",
    "reason": "",
    "recorded_at": "2026-08-04T13:44:22.173Z",
    "resolved_at": "2026-08-08T16:46:36.049Z"
  }
]
````
