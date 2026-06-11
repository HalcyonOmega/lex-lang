# 02 — Syntax Decisions (the owner's control surface)

**The owner has final say on all user-facing syntax.** Agents implement
only what is Ratified, may rely on Provisional choices (clearly marked,
reversible), and must never invent surface syntax. To propose something
new: add a row to Open Decisions with options and tradeoffs, and stop.

How to ratify: move the row to Ratified with your chosen option. Agents
then update `src/syntax.rs` (and parser if structural), re-bless ui
snapshots (`UPDATE_EXPECT=1 cargo test`), and update docs/01-spec.md.

## Ratified

**N1 — Language name** *(ratified 2026-06-11)*: **Lex**. Binary: **`lex`**.
Rejected: Jet, Cove, Olex-as-public-name.

**N2 — File extension** *(ratified 2026-06-11)*: **`.lex`**. Source files
are `name.lex`; the extension matches the language name (three letters).

**S1 — Function keyword** *(ratified 2026-06-11)*: **`fn`**. Rejected:
`func`, `def` — recognized only as foreign syntax to emit a teaching
error pointing at `fn` (see S14).

**S3 — Blocks** *(ratified 2026-06-11)*: **curly braces `{ }`**. Rejected:
`end` keywords, significant indentation.

**S8 — String interpolation** *(ratified 2026-06-11)*: **`"hi {name}"`**
— expressions inside `{ }` within quoted text (modern standard). Rejected:
`"hi " + name` concatenation (no `+` for strings; one obvious way).

**S9 — Print builtin** *(ratified 2026-06-11)*: **`print`** (adds a
newline). Rejected: `println` — recognized only as foreign syntax (S14).

**S11 — Built-in type names (M1)** *(ratified 2026-06-11)*: capitalized
**`Int`**, **`Float`**, **`Bool`**, **`String`**. Rejected: `Text`
(industry uses `String`; `Text` recognized only as foreign syntax per
S14), lowercase `int`/`text`.

**S2 — Variable bindings (M1)** *(ratified 2026-06-11)*: **`val`** for
immutable bindings, **`var`** for mutable bindings. Rejected: `set`
(sounds like mutation), `let` / `let mut` (Rust; teaching errors only per
S14).

**S18 — Visibility** *(ratified 2026-06-11)*: **private by default**;
prefix **`pub`** to export an item. Applies to top-level functions (M0+),
types and their fields (M3), and any future module-level bindings.
Within a file, private and `pub` items are equally visible to each other;
`pub` only controls what other files may access via `import` (S16, M6+).
Rejected: public-by-default (Go), explicit `private` keyword (noisy).

**S10 — Ownership keywords (M2)** *(ratified 2026-06-11)*: **`mut`**
(mutable borrow), **`take`** (move), **`view`** (borrow return type),
**`ref`** (stored field, tier 2). Default parameter access has no keyword
(shared read). Rejected: `read` / `write` / `owned` as canonical forms.

**S14 — Alias policy** *(ratified 2026-06-10)*: One canonical spelling per
construct; **no aliases, ever**. v1: the compiler recognizes common foreign
syntax (`and`, `try`, `let`, `set`, `func`, `def`, `println`, `Text`, …) and the error
teaches the canonical form.
Later (M6): the LSP offers an autocorrect quick-fix for foreign syntax and
`fmt` canonicalizes, so non-canonical input never survives to disk. True
dual forms are rejected permanently.

## Provisional — currently in the code

| ID  | Choice in code         | Where                |
|-----|------------------------|----------------------|
| S5  | `//` comments          | src/syntax.rs        |
| S6  | semicolons             | src/syntax.rs, parser  |
| S7  | `?` suffix             | src/syntax.rs        |
| S12 | entry point `main`     | sema                 |
| S13 | `&&` `||` `!` `==` `!=` `<` `>` `<=` `>=` | src/syntax.rs |
| S17 | `+=` `-=` `*=` `/=` `%=` `&=` `|=` `^=` `<<=` `>>=` | src/syntax.rs |
| S16 | `import "path" as alias` | src/syntax.rs      |

## Open decisions — owner input needed

**S4. Type annotations (M1).** `x: Int` after the name (Rust/Swift/TS,
plays well with inference) / `Int x` before (C/Java). Provisional: `name: Type`.

**S5. Comments.** `//` / `#`. Provisional: `//`.

**S6. Statement separators.** None — newline-ish (cleanest to read,
slightly trickier grammar later) / semicolons (unambiguous, C++/Rust
familiar). Provisional: **semicolons** (required between statements;
optional before a closing `}`). Ratify early — this one gets harder to
change after M1 expressions land.

**S7. Error propagation (M4).** Suffix `?` (terse, Rust-familiar) / prefix
`try` (reads in order: `try parse(x)`) / explicit `match` only (maximal
clarity, maximal boilerplate). Provisional: **`?` suffix** — e.g.
`parse_int(raw)?`. Prefix `try` recognized only to emit a teaching error
(see S14).

**S12. Entry point.** `main` (universal convention) / top-level
statements allowed, no main needed (friendliest possible hello-world,
complicates the "everything is a function" story). Provisional: `main`.

**S13. Logical and comparison operators (M1).** `and` / `or` / `not`
(Stefik: word forms help novices) / `&&` / `||` / `!` with `==` `!=`
`<` `>` `<=` `>=` (C-family, familiar from Rust/C++/JS). Provisional:
**symbols** — `&&` `||` `!` for logic; `==` `!=` for equality; `<` `>`
`<=` `>=` for ordering. Word forms (`and`, `or`, `not`) recognized only
to emit a teaching error (see S14).

**S14. Alias policy.** DIRECTION SET (owner): **no true aliases.** There
is one canonical spelling of everything. Familiar-from-other-languages
forms (`and`, `try`, `let`, `set`, `func`, `def`, `println`) are recognized by
the parser ONLY to emit a teaching error naming the canonical form ("in
Lex, write `fn` — fix: replace `def` with `fn`"). Later, the LSP/`fmt`
may auto-rewrite these to the canonical form on save (autocorrect), once
the formatter exists and runs by default (M6). Rejected: keeping both
forms in real code (creates dialects; beginners must read both anyway).
Build order: error-first now (M1), LSP autocorrect later (M6). Reserve
error codes E0008+ for these.

**S17. Compound assignment (M1).** Full C-family set: **`+=` `-=` `*=` `/=`
`%=` `&=` `|=` `^=` `<<=` `>>=`** (`x += 1` means `x = x + 1`) / simple
`=` only (force `x = x + 1` everywhere — one form, noisier). Provisional:
all ten operators. **`+=` `-=` `*=` `/=`** on `Int` and `Float`; **`%=`
`&=` `|=` `^=` `<<=` `>>=`** on `Int` only. Left-hand side must be a
mutable binding (`var`) or `write` parameter.

**S16. Module imports (multi-file, M6+).** How one file pulls in another
when the opt-in package/multi-file story lands (Architecture R9). Options:
`import "path" as alias` (Zig-like: path string + local name via `as`) /
Rust `use path::item` (granular, unfamiliar to beginners) / Python
`import module` (no path string, package layout magic). Provisional:
**`import "path" as alias;`** — e.g. `import "grades/scoring" as scoring;`
then call `scoring.letter_grade(x)`. Path is a quoted string relative to
the project root (exact resolution deferred to M6 tooling). `func`/`def`
are not import syntax; `use` / bare `import` recognized only for S14
teaching errors when M6 lands.

**S15. Binary profile / panic strategy.** Default build = speed-leaning
(`-O`, strip, thin LTO) per Architecture R8. Open: should there be a
`lex build --small` profile (`opt-level="z"`, full LTO) and should it (or
even the default) use `panic=abort`? `abort` shrinks binaries and is
arguably fine for a beginner language, but removes unwinding/`catch`
semantics — a real behavior change. Provisional: default keeps unwinding;
`--small` profile deferred to M6 tooling. Owner to confirm panic stance.

## Decision log

| Date       | ID  | Decision                          | By   |
|------------|-----|-----------------------------------|------|
| 2026-06-11 | N1  | Lex; binary `lex`                 | owner |
| 2026-06-11 | N2  | extension `.lex`                  | owner |
| 2026-06-11 | S3  | `{ }` blocks                      | owner |
| 2026-06-11 | S8  | `"text {expr}"` interpolation     | owner |
| 2026-06-11 | S9  | `print` (not `println`)           | owner |
| 2026-06-11 | S2  | `val` / `var` (not `set` or `let`)   | owner |
| 2026-06-11 | S18 | private by default; `pub` to export    | owner |
| 2026-06-11 | S11 | `String` (not `Text`); `Int` `Float` `Bool` | owner |
| 2026-06-11 | S1  | `fn` (not `func` or `def`)        | owner |
| 2026-06-11 | S10 | `mut` / `take` / `view` / `ref`   | owner |
| 2026-06-10 | S14 | no true aliases; teach foreign forms | owner |
