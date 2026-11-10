# Offer to bring a non-effective BRR to effective

Used from step 3 of `badr-design-arch` when a **needed** BRR is `draft`,
`proposed`, or `approved` (delivers BRR-ARC-007). These are the only writes this
skill may perform, and each is a separate, individually authorized BADR command.

## The ladder

| BRR status | Next step | Command |
| --- | --- | --- |
| draft | **Propose** | `badr --repo <repo> propose <id> --actor <actor> --rationale <text> --confirm` |
| proposed | **Decide** (approve) | `badr --repo <repo> decide <id> --actor <actor> --rationale <text> --reviewed --confirm` |
| approved | **Activate** | `badr --repo <repo> activate <id> --actor <actor> --rationale <text> --confirm` |

Promotion runs only in **Full** mode; in a blocked or limited session, never offer it.
Offer only the steps the BRR still needs, in order. Never offer a later step
before the earlier one is done and verified (`badr show <id> --json`).

## Rules

1. **One step, one authorization.** Ask for each step separately. Authorization
   for propose is not authorization for decide or activate; approval is **never**
   treated as activation. Do not batch steps or pre-approve a ladder.
2. **Explicit, per step.** The user must say yes to that step, and give the
   **actor** and **rationale** for it. If either is missing, ask; do not invent
   a rationale. Silence, enthusiasm for the design, or a prior step's approval
   is not authorization. Without it, the step does not run.
3. **Human review for decide.** Pass `--reviewed` only if the user states they
   reviewed the BRR's content and gaps. Do not claim this on their behalf.
4. **Run, then verify.** After each step, re-read the BRR with
   `badr show <id> --json` and report the new status. Stop on any non-zero exit
   and report diagnostics; do not retry with changed arguments.
5. **Only an `effective` BRR drives.** Until activation succeeds, the BRR stays
   a pending item and is not a driver.

## Declining

If the user declines a step (or the whole offer), stop offering for that BRR.
Continue the design with every decision that rests on it labelled **not
BRR-backed**, keeping the BRR's ID and status as context. Do not ask again in
the session unless the user raises it.
