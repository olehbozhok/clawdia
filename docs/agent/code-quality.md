# Code Quality

- **No corner-cutting.** This code runs in production. Every feature, error path, edge case, and test specified in the plan must exist. Replacing a planned component with a simpler one without justification is not acceptable. Deferring functionality that the plan explicitly includes (e.g. "d deny opens a reason modal") requires the user's approval.
- Follow best practices for the language being used (idiomatic Rust, Pythonic Python, etc.).
- Always evaluate whether a piece of functionality should be extracted into a separate module, function, or file. Consider separation of concerns, reusability, and readability.
- Keep functions and modules focused on a single responsibility.
- Prefer clear, descriptive naming over comments.
- Prefer named structs over tuples for return types and parameters when there are 2+ fields. Tuples like `(ServerSink, Vec<Tool>)` are opaque — use a struct with named fields instead.
- In Rust, never use `Box<dyn std::error::Error>` for error handling. Use `thiserror` for library error types and `anyhow::Result` for application-level code.
- Never use byte-index slicing (`&s[..n]`) on strings. This panics on multi-byte UTF-8 characters. Always use `.chars().take(n)` or `char_indices` for truncation.
- Any background async task (`tokio::spawn` with an unbounded loop, periodic sweep, watcher) **must** accept a `tokio_util::sync::CancellationToken` and exit on `cancel.cancelled()` via `tokio::select!`. Returning a bare `JoinHandle` without a cancel path forces callers to `abort()` and leaks in-flight work. Test the cancel path with a bounded `tokio::time::timeout`.
- **Preserve named constants.** Never inline a named constant (e.g. `DEFAULT_MAX_TURNS`) into its literal value (e.g. `40`). Constants exist for a reason: single source of truth, self-documenting intent, and easy tuning. When refactoring, keep the constant definition and reference it — do not delete it and scatter its value as magic numbers.
