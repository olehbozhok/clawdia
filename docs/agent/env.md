# Environment Files

- Always maintain an `.env.example` file alongside any `.env` file in the project.
- `.env.example` must contain all required environment variable names with placeholder values (e.g. `API_KEY=your-api-key-here`).
- When adding, removing, or renaming environment variables in code, immediately update `.env.example` to keep it in sync.
- When writing code that reads environment variables, validate that all required variables are set and non-empty at startup. Provide a clear error message listing any missing variables.
- All environment variables must be read in `main` (or the top-level entry point) and passed into functions as explicit parameters. Functions must never call `std::env::var` or equivalent directly — this keeps the source of configuration obvious and testable. In Rust CLI apps using clap, prefer `#[arg(env = "VAR_NAME")]` to read env vars declaratively in the CLI struct rather than manual `std::env::var` calls.
