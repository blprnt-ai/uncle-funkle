---
name: uncle-funkle
description: Use this skill when working with the uncle-funkle Rust CLI to scan a repository for maintainability debt, inspect saved issue state, list open issues, move issues through resolve/defer/dismiss/reopen workflows, or consume JSON output in an agent-driven cleanup loop.
---

# Using Uncle Funkle

Use this skill when you need to inspect or progress code-quality cleanup work with `unfuk` or `npx @blprnt/unfuk`.

Supported npm-distributed prebuilt targets: darwin-arm64, linux-x64, and win32-x64.

## What The Tool Does

`unfuk` scans a repository for maintainability issues, stores them as persistent issues in `.uncle_funkle/state.json`, scores the codebase, and surfaces the next item to fix.

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
unfuk scan
unfuk scan path/to/project
npx @blprnt/unfuk scan
```

Behavior:

- scans the project
- merges findings into saved state
- recomputes scores
- returns a summary and the current next item

### Status

```bash
unfuk status
unfuk status path/to/project
unfuk status path/to/project ISSUE_ID
```

Behavior:

- loads saved state without rescanning
- shows the current next item by default
- shows a specific issue when `ISSUE_ID` is provided

### List

```bash
unfuk list
unfuk list path/to/project
unfuk list path/to/project --all
```

Behavior:

- lists open issues by default
- includes all statuses with `--all`

### Progress Work

```bash
unfuk next
unfuk resolve ISSUE_ID
unfuk defer ISSUE_ID
unfuk dismiss ISSUE_ID
unfuk reopen ISSUE_ID
unfuk resolve ISSUE_ID path/to/project
```

Behavior:

- `next` resolves the current next issue and returns the new next item
- the other commands update a specific issue directly

## Recommended Agent Workflow

1. Run `unfuk scan <path>`.
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
