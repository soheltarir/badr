# Decision selection and ADR conflicts

Used after the handoff in `badr-design-arch`. Nothing here writes a file; the
output is the set of **user-accepted decisions**, saved by [Save accepted
decisions](saving.md).

## Candidates

1. Derive candidate decisions from the drivers: effective BRRs and any
   **not BRR-backed** free-form items. For each give a short title, the drivers
   it answers, and the question it settles.
2. List all candidates and ask which to pursue. Only user-selected candidates
   proceed. Unselected candidates are dropped: no ADR, no draft, no later
   reminder. Never add a decision the user did not select.

## Presenting a selected decision

Before the user chooses, show for each selected decision:

- at least **two real alternatives**, each viable under the drivers, with its
  trade-offs (no strawmen added to pad the count; if only one option is
  viable, say so and ask for more input rather than inventing one);
- a **recommendation** with reasons tied to specific drivers;
- the related ADRs and any overlap (below).

The user then chooses an alternative, which need not be the recommendation.

## Related ADRs and overlap

Before judging any conflict, read every related ADR **in full** with
`badr --repo <repo> show <id> --json`, from the allowed sources only
(see [Preflight and sources](preflight.md)). Find them with
`badr --repo <repo> list --type adr --json` and `list --directing --json`.
Titles or summaries are not enough. Overlap means the same question is settled,
or the chosen option contradicts or narrows an existing outcome.

| Overlapping ADR status | Handling |
| --- | --- |
| **approved** | Stop. Cite ID, title, and path, and the overlapping text. The user chooses exactly one: **keep** it (drop or reshape the new decision), write a **successor** that replaces it, or record a **separate linked ADR** that coexists and references it. Do not choose for them or continue until they answer. |
| **proposed** | Report ID, title, path, and the overlap; do not block. Continue. |
| rejected, deprecated, superseded | Mention as history only if relevant. |

Proposing a
successor or link records intent only; do not approve, supersede, or deprecate
anything here.
