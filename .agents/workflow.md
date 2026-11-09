# Workflow

## Before the change

- Read `AGENTS.md` and the matching file in `.agents/`.
- Read the affected code before you change it. Follow the existing patterns (take a similar plugin as the template).
- Clarify unclear requirements briefly, or pick the most obvious reading and name it in the PR.

## During the change

- One topic per branch and PR. Do not rebuild or reformat anything on the side.
- Versions: newest version of Node.js, Rust, dependencies and actions (see basic rule 6 in `AGENTS.md`).
- No secrets, tokens or personal paths in the repo.

## Before the commit

```sh
npm run check
npm run build
cargo test --workspace
```

All three must be green, just like in the CI. If something could not run (e.g. missing system libraries), say so in the PR.

- Update the README when something changes for users or developers.
- Update these rules (`AGENTS.md`, `.agents/`) when structure, commands or conventions change. They must always match the code.

## Commits and PRs

- Commit messages in English, imperative mood, one summary line (e.g. "Add kanban board as a core plugin"), and if needed a short paragraph with the why.
- PR description: what changes, why, how it was checked. Visible changes with a screenshot.
- Never push directly to `main`, never rewrite other people's branches, never disable tests to get the CI green.
