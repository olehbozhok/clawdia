# Adding Information to CLAUDE.md

When the user asks to add something to CLAUDE.md:

- First check if the information already exists (in CLAUDE.md or `docs/agent/*.md`) to avoid duplication.
- If it's a detailed rule, put it in the appropriate `docs/agent/*.md` file, not in CLAUDE.md.
- Only add to CLAUDE.md if it's universally critical (like Security).
- Follow existing section style and placement.
- **Do not update AGENTS.md separately** — it is a symlink pointing to CLAUDE.md, so updating CLAUDE.md is sufficient.
