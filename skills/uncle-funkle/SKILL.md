---
name: uncle-funkle
description: Use this skill when working with the uncle-funkle Rust CLI to scan a repository for maintainability debt, inspect saved issue state, list open issues, move issues through resolve/defer/dismiss/reopen workflows, or consume JSON output in an agent-driven cleanup loop.
---

# Using Uncle Funkle

Use this skill when you need to inspect or progress code-quality cleanup work with `uncle-funkle`.

## What The Tool Does

`uncle-funkle` scans a repository for maintainability issues, stores them as persistent issues in `.uncle_funkle/state.json`, scores the codebase, and surfaces the next item to fix.

It is useful for:

- scanning a repo before cleanup work
- checking saved state without rescanning
- listing open issues
- progressing issue state through the CLI
- consuming machine-readable JSON output in an agent workflow

## Commands

Most commands default to the current directory when no path is provided.

### Scan

```bash
uncle-funkle scan
uncle-funkle scan path/to/project
```

Behavior:

- scans the project
- merges findings into saved state
- recomputes scores
- returns a summary and the current next item

### Status

```bash
uncle-funkle status
uncle-funkle status path/to/project
uncle-funkle status path/to/project ISSUE_ID
```

Behavior:

- loads saved state without rescanning
- shows the current next item by default
- shows a specific issue when `ISSUE_ID` is provided

### List

```bash
uncle-funkle list
uncle-funkle list path/to/project
uncle-funkle list path/to/project --all
```

Behavior:

- lists open issues by default
- includes all statuses with `--all`

### Progress Work

```bash
uncle-funkle next
uncle-funkle resolve ISSUE_ID
uncle-funkle defer ISSUE_ID
uncle-funkle dismiss ISSUE_ID
uncle-funkle reopen ISSUE_ID
uncle-funkle resolve ISSUE_ID path/to/project
```

Behavior:

- `next` resolves the current next issue and returns the new next item
- the other commands update a specific issue directly

## Recommended Agent Workflow

1. Run `uncle-funkle scan <path>`.
2. Read the summary and current next item.
3. Inspect and edit code outside this tool.
4. Use `resolve`, `defer`, `dismiss`, `reopen`, or `next` to update state.
5. Re-run `scan` after code changes.

## Best Practices

- Run `scan` before trusting state in a repo you have not seen before.
- Use `status` when you want saved state only and do not want to rescan.
- Treat issue IDs as stable handles for follow-up commands.
- Use `list --all` when auditing historical cleanup progress.
- Do not assume `next` means “finish the whole plan.” It advances one issue.
