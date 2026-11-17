---
name: concept-guard
summary: Check approved requirement anchors before changes; resolve Concept divergences with human decisions.
---

When a project prompt arrives, the UserPromptSubmit hook already evaluates it. Do not re-read the entire documentation library. If the hook reports divergence, give the human: (1) requirement ID/file/line/excerpt, (2) new request or code evidence, (3) brief mismatch, (4) four actionable options. Do not claim that a heuristic is definitive semantic proof.

Use MCP `requirements_get`, `requirements_check`, and `concept_discuss` for precise evidence. `concept_decide` creates a proposal only. Never claim a MCP call authorized a baseline change; a human TTY decision and protected PR are required. Basic design, detailed design, code, and tests remain normal development artifacts, not new governed anchors. Do not automatically modify requirement documents.
