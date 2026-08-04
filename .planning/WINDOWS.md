---
schema_version: 1
open_count: 2
waived_count: 0
fixed_count: 0
total_count: 2
last_updated: 2026-08-04T03:16:35.305Z
---

# Broken Windows Ledger

> Cross-phase defect register. `/gsd-ship` blocks while `open_count > 0`.
> Waive with `gsd-tools windows waive <id> "<reason>"` (reason required).
> Mark fixed with `gsd-tools windows fixed <id>`.

| id | phase | kind | file | line | description | status | reason | recorded_at | resolved_at |
|----|-------|------|------|------|-------------|--------|--------|-------------|-------------|
| 1 | 51 | unrun-verify | cli/caprun/tests/live_acceptance_v1_10_cli.rs |  | LIVE-07 compose-verify unavailable because Docker is not installed on executor host | open |  | 2026-08-04T03:11:17.796Z |  |
| 2 | 51 | unrun-verify | cli/caprun/tests/live_acceptance_v1_10_cli.rs |  | LIVE-08 scoped and full-workspace compose verification unavailable because Docker is not installed on executor host | open |  | 2026-08-04T03:16:35.305Z |  |

````json
[
  {
    "id": 1,
    "kind": "unrun-verify",
    "phase": "51",
    "file": "cli/caprun/tests/live_acceptance_v1_10_cli.rs",
    "line": null,
    "description": "LIVE-07 compose-verify unavailable because Docker is not installed on executor host",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-08-04T03:11:17.796Z",
    "resolved_at": null
  },
  {
    "id": 2,
    "kind": "unrun-verify",
    "phase": "51",
    "file": "cli/caprun/tests/live_acceptance_v1_10_cli.rs",
    "line": null,
    "description": "LIVE-08 scoped and full-workspace compose verification unavailable because Docker is not installed on executor host",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-08-04T03:16:35.305Z",
    "resolved_at": null
  }
]
````
