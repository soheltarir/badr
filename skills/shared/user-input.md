# Structured user input

Apply this reference whenever a BADR skill asks the user a question, conducts an
interview, requests a choice, or seeks confirmation or authorization.

Use the host's native structured user-input tool so the user sees a rendered
question or form: for example, Codex `request_user_input`, Claude Code
`AskUserQuestion`, OpenCode `question`, or an equivalent exposed by the host.
Use the tool's advertised schema and current-mode restrictions; a Markdown list
or a tool-call example in prose does not render an input.

Give concrete choices with short labels and useful descriptions. Mark a
recommendation when one is justified. For open-ended answers, use a supported
text field or the host's custom-answer facility; do not invent choices merely
to fit a multiple-choice schema. Show lengthy context or exact record content
in the conversation, then collect the answer through the rendered input.

Batch independent frontier questions up to the tool's limits. Ask dependent
questions after their prerequisites are answered. Wait for submitted answers;
defaults, recommendations, timeouts, dismissal, and silence are not answers or
authorization. Preserve each workflow's existing exact-content and per-operation
authorization requirements.

If the reference or a suitable tool is unavailable, the current mode forbids
it, or rendering fails, report the limitation and leave the question pending.
Continue only work independent of that answer. Resume the question when the
native input is available; use a plain-text fallback only if the user explicitly
requests it. Claim an answer or successful interaction only from the host's
returned result.
