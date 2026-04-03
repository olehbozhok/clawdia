# Project Instructions

## Security

- NEVER read, display, or log the contents of `.env` files or any file containing secrets (API keys, passwords, tokens).
- NEVER include real secret values in code, comments, logs, or tool output.
- When creating `.env` files, always use placeholder values like `your-key-here`.
- If you need to reference environment variables, refer to them by name only (e.g. `DEEPSEEK_API_KEY`), never by value.

## Environment Files

- Always maintain an `.env.example` file alongside any `.env` file in the project.
- `.env.example` must contain all required environment variable names with placeholder values (e.g. `API_KEY=your-api-key-here`).
- When adding, removing, or renaming environment variables in code, immediately update `.env.example` to keep it in sync.
- When writing code that reads environment variables, validate that all required variables are set and non-empty at startup. Provide a clear error message listing any missing variables.

## Code Quality

- Follow best practices for the language being used (idiomatic Rust, Pythonic Python, etc.).
- Always evaluate whether a piece of functionality should be extracted into a separate module, function, or file. Consider separation of concerns, reusability, and readability.
- Keep functions and modules focused on a single responsibility.
- Prefer clear, descriptive naming over comments.

## Validation

- Always use `uv run pyright` to validate Python code instead of running the app, when possible.
- Fix all pyright errors before considering the task complete.
