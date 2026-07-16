# AGENTS.md

This file provides repository-level guidance to Codex.

## Repository context

- Review `CLAUDE.md` for project architecture, commands, and conventions before making code changes.

## Git workflow

- Before creating any git commit, always ask the user for approval and wait for their response.
- At the end of every turn that modifies files, offer to create a git commit for the current task.
- Before committing, run `git status --short` and review the diff.
- Stage only files changed for the current task. Do not stage unrelated user changes.
- Use a concise imperative commit message.
- If no files were modified, do not create a commit.
- If required checks fail, do not commit unless the failure is unrelated or explicitly accepted; explain the reason before ending the turn.

## Release notes and versioning

- Ask the user whether the current task should be added to the changelog.
- Remember to update the fourth digit of the version number when preparing a version update.
