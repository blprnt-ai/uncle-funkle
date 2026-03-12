# Uncle Funkle

`uncle-funkle` is a Rust library for scanning a codebase for maintainability problems, storing them as persistent issues, scoring the project, and generating a prioritized cleanup plan.

It focuses on practical heuristics like leftover TODOs, debug artifacts, oversized files, long functions, deep nesting, branch-heavy code, long lines, and duplicate blocks.

## What It Does

- scans source files across several common languages
- records findings as stable issues with lifecycle state
- persists project state to disk
- recomputes quality scores after each scan or review import
- generates a ranked plan for cleanup work
- supports subjective assessment imports alongside mechanical findings

## How It Works

The library follows a simple loop:

1. scan the project
2. merge findings into persisted state
3. recompute scores and stats
4. build a prioritized cleanup plan
5. save the updated state

State is stored under `.uncle_funkle/state.json` by default.

## Current Shape

This repo is a library crate, not a CLI application.

The public surface is built around:

- creating an engine with config
- loading and saving state
- scanning a project
- merging scan output
- importing subjective assessments
- generating the next cleanup plan item
- resolving, deferring, dismissing, or reopening issues

## Defaults

By default the scanner:

- includes common source file extensions like Rust, Python, JavaScript, TypeScript, Go, C#, Dart, Java, Kotlin, C, C++, Ruby, and Swift
- skips generated or irrelevant directories like `.git`, `target`, `node_modules`, `build`, `dist`, `coverage`, virtualenvs, vendors, and its own state directory
- skips oversized or binary files

Default thresholds include:

- long line: 120 characters
- large file: 400 lines
- long function: 80 lines
- deep nesting: 4 levels
- branch density: 10 branch points
- duplicate block window: 6 lines

## Scores

The library maintains four scores:

- `objective` — based on active mechanical findings
- `strict` — harsher score with extra penalty for severe and reopened issues
- `overall` — blends objective and subjective review when available
- `verified` — the stricter final view when subjective review exists

## Status

What it is:

- a compact library for heuristic code quality triage
- persistent issue tracking across scans
- a scoring and cleanup-planning engine

What it is not:

- not a CLI yet
- not an AST-heavy static analyzer
- not an auto-fixer
- not a security scanner

## Verification

Suggested local checks are listed in `VERIFY.md`.

## License

MIT