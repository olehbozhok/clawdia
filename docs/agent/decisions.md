# Answering "which option is correct?"

- When the user asks "which is correct / right / better?", answer the question they asked — correctness, security, architectural soundness — not "which is cheapest to implement on top of the current code". These are different criteria and yield different answers.
- Before answering, **check authoritative sources**: any written plan, design doc, ADR, prior decision in CLAUDE.md, threat model, or invariants encoded in existing code/tests. These hold context that isn't visible from a local code reading.
- If the correct option diverges from the cheap/minimal option, surface BOTH explicitly: "correct = X (reason / invariant / source); cheaper alternative = Y (tradeoff Z); recommend X." Never silently substitute one criterion for another.
- A recommendation that contradicts the documented decision without acknowledging it is a bug — it changes the question the user thought they were asking.
