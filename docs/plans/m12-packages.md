# M12 — Package manager

**Blocked on decisions:** S52 (manifest format & commands), S51 (import
spelling interplay). Depends on M6 (multi-file), M7 (FFI deps belong in
the manifest), M10 (std as the precedent for namespaced imports).
**Error codes:** E1201+.

## Goal

Opt-in packages with zero ceremony for the single-file case (R9 is
sacred: `lex run file.lex` never needs any of this). Phase 1 ships
path + git dependencies with a lockfile; a registry is phase 2 behind a
trivially-hostable static index. Lex packages are Lex source compiled
from scratch — no binary artifacts, no build scripts, no install hooks
(supply-chain stance: a dependency can't run code at install time).

## Surface (uses ballot recommendations — substitute ratified choices)

`lex.toml` (S52 — TOML, hand-parsed in the compiler, I6):

```toml
[package]
name = "wordstats"
version = "0.1.0"

[dependencies]
textkit = { git = "https://github.com/someone/textkit", tag = "v1.2.0" }
helpers = { path = "../helpers" }

[rust-dependencies]        # M7 FFI pins move here when a manifest exists
base64 = "0.22"
```

```lex
import words;                            // package dep (module under pkg cache root)
import "pkg/textkit/words" as words;    // explicit path (still valid)
import scoring;                         // local module by name (S16)
import "grades/scoring";                // local file path, default ns scoring
```

Commands:
- `lex add textkit --git <url> --tag v1.2.0` / `lex add helpers --path ../helpers`
  — edits lex.toml, resolves, updates `lex.lock`.
- `lex fetch` — resolve + download everything in lex.toml into
  `~/.cache/lex/pkg/<name>/<rev>/`; writes/verifies `lex.lock`
  (exact revisions + content hashes).
- `lex run` / `build` / `test` auto-detect lex.toml upward from the
  entry file; no manifest found = single-file mode, exactly as today.
- `lex publish` is **phase 2** (registry); not in this milestone's exit.

## Rules

1. A package's importable surface is its `pub` items (S18); the package
   root is the directory containing its lex.toml; `pkg/<name>/<path>`
   maps to files inside the dependency exactly like M6 local imports.
2. Version conflicts: v1 policy is **one version per package name** in
   the whole graph; conflicting requirements → E1201 showing both
   requirement chains ("`a` wants textkit v1, `b` wants textkit v2").
   No silent duplication (smallness; revisit only with evidence).
3. Lockfile is authoritative: building with a lock mismatch → E1202
   ("run `lex fetch`"); CI-friendly `--locked` refuses network.
4. Git fetching shells out to `git` (no network code in the compiler;
   `git` missing → E1203). Content hashes (sha256 of the file tree)
   recorded in lex.lock and verified on every build (E1204 tamper
   error). Hash implementation: vendored pure-Rust sha256 in-tree
   (~100 lines, no crate — I6).
5. `[rust-dependencies]` feeds the M7 cargo bridge; inline `@version`
   pins in `extern rust` become E1205 ("this project has a manifest;
   pin versions in lex.toml") when a manifest exists.
6. Dependency diagnostics render with the package name in the path
   (`textkit/words.lex:14:3`) but a program never fails because of a
   *warning* in a dependency (lints are suppressed outside the root
   package).
7. Manifest parse errors are E1206 with line/column and the same
   diagnostic voice (the TOML subset parsed: tables, strings, inline
   tables — reject anything else with "lex.toml only uses simple
   `key = value` lines and `[sections]`").

## Phase 2 — registry (separate agent run; exit criteria split)

A static-index registry in the sigstore-less cargo style: a git repo
(`lex-lang/registry`) of JSON lines per package (name, versions, git
URL, tree hash). `lex add textkit` consults the index; `lex publish`
opens a PR-able entry (manual review while the ecosystem is tiny —
honest and cheap). No server to run; promote to a real API only on
demonstrated need.

## Diagnostics to register

E1201 version conflict (both chains) · E1202 lockfile out of date ·
E1203 git not installed · E1204 dependency hash mismatch · E1205
FFI pin belongs in lex.toml · E1206 manifest syntax/shape error.
Teaching: E0042 `cargo.toml`/`package.json` filename → `lex.toml` ·
E0043 `lex install` → `lex fetch`.

## Examples & tests

- `tests/pkg/` — fixture workspaces (root + path deps + a vendored
  fake "git" dep using a local bare repo created by the test) covering:
  clean build, lock verification, version conflict, private-item
  import (E0605 across packages), tamper detection.
- End-to-end: `lex new` + `lex add --path` + `lex run` in a tempdir.
- Doc: docs/07-packages.md (user-facing guide) written this milestone.

## Out of scope

Semver resolution/ranges (exact pins only in phase 1), binary caching of
compiled deps (rebuild from source; cache generated .rs by hash later if
slow), namespaces/scopes, yanking, registry mirrors, workspaces /
monorepos, dev-dependencies (tests in deps just don't run), features/
conditional compilation.
