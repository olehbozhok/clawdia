# Project Instructions

## Security

- NEVER read, display, or log the contents of `.env` files or any file containing secrets (API keys, passwords, tokens).
- NEVER include real secret values in code, comments, logs, or tool output.
- When creating `.env` files, always use placeholder values like `your-key-here`.
- If you need to reference environment variables, refer to them by name only (e.g. `DEEPSEEK_API_KEY`), never by value.

## Validation

- Always use `uv run pyright` to validate Python code instead of running the app, when possible.
- Fix all pyright errors before considering the task complete.
