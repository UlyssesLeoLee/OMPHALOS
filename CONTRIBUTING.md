# Contribution rules
- Approved requirements sources and `.concept-guard/approved.json` are subject to an explicit human-owner review, even when the line changes look trivial.
- Never auto-approve Concept changes from MCP tools or an AI agent.
- PreToolUse hooks and local git hooks are advisory; protected branch rules and required CI checks provide the merge gate.
- `concept-guard init` writes `pending.json`. Only a human terminal can run `approve`.
- A changed requirements definition must be followed by new approved baseline created on an explicitly reviewed PR; changing both source and baseline must not bypass required owner review.
- Use `scripts/pre-commit` as an optional local helper. It is not tamperproof.
