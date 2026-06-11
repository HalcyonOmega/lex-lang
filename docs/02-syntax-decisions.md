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

**S6 — Statement separators** *(ratified 2026-06-11)*: **semicolons,
required after every statement** — including the last statement before a
closing `}`. One rule, no exceptions. Rejected: newline separators,
optional-before-`}`.

**S12 — Entry point** *(ratified 2026-06-11)*: **`fn main()`** — a special
case; no `pub` required (the runtime always finds `main`). Canonical form
omits `pub`. Rejected: required `pub fn main` (ceremony), top-level
statements without a main.

**S19 — Loops (M1)** *(ratified 2026-06-11)*: **`while cond { }`** and
**`for i in <range> { }`**. Rejected: recursion-only M1, `loop` + `break`
as the primary construct.

**S22 — Range bounds (M1)** *(ratified 2026-06-11)*: **`1..10` is
inclusive** — it counts 1 through 10. Reads like English, kills the classic
beginner off-by-one. M5 slicing may bring its own evidence; revisit there
if needed. Rejected: half-open `..` (Rust/Python), dual `..`/`..=`, word
form `1 to 10`.

**S23 — Loop control (M1)** *(ratified 2026-06-11)*: **`break`** (leave
the loop now) and **`continue`** (skip to the next turn). Rejected:
plain-word `stop`/`skip`, omitting loop control from M1.

**S24 — Many-way choice: `switch` (M1)** *(ratified 2026-06-11)*:

```
switch x {
    x == 1 -> { ... };
    x == 2 || x == 3 -> { ... };
    else -> { ... };
}
```

Keyword **`switch`**; the head expression names the subject being
examined; each arm is a full `Bool` condition, then `->`, then a `{ }`
block, ended with `;` (S6). The first true arm runs; **an `else` arm is
required**. Arms are ordinary conditions, so ranges and compound tests
need no special pattern syntax (`x >= 400 && x <= 499 -> { … };`).
The backend lowers subject-equals-literal chains to a native Rust `match`
(jump tables where profitable) and everything else to an if/else chain —
optimization is the compiler's job, never the user's. Rejected: C
`switch`/`case`/`default` (fallthrough baggage), bare-value `match`
(`match` is recognized only for an S14 teaching error). M3's enum
exhaustiveness story extends `switch`.

**S20 — Escapes & literal braces (M1)** *(ratified 2026-06-11)*: minimal
escape set **`\n` `\t` `\"` `\\`**; literal braces are written by doubling:
**`{{`** for `{` and **`}}`** for `}` (Rust/Python style). A lone `}` in
quoted text is an error teaching `}}`. More escapes (`\r`, `\u{…}`) wait
for demand. Rejected: `\{`, full C escape set.

**S21 — Float display (M1)** *(ratified 2026-06-11)*: a `Float` always
prints with a decimal part — `-5.0` prints `-5.0`, never `-5`. The value
visibly stays a Float. Rejected: Rust's `Display` default (drops `.0`).

**S25 — Comparison distribution (M1)** *(ratified 2026-06-11)*: in a
`&&`/`||` chain, when the right side is a plain value instead of a yes/no,
the nearest comparison to its left is re-applied to it:
`day == "mon" || "tue"` means `day == "mon" || day == "tue"`. Works for
chains (`x == 1 || 2 || 3`) and every comparison operator
(`x != 1 && 2`). The value's type must match what was compared. When the
values really are different things, write the full comparisons as usual.
Rejected: always requiring full repetition (noisy), a set-membership
construct like `x in (1, 2)` (a whole new form for the same idea).

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
| S7  | `?` suffix             | src/syntax.rs        |
| S13 | `&&` `||` `!` `==` `!=` `<` `>` `<=` `>=` | src/syntax.rs |
| S17 | `+=` `-=` `*=` `/=` `%=` `&=` `|=` `^=` `<<=` `>>=` | src/syntax.rs |
| S16 | `import "path" as alias` | src/syntax.rs      |

## Open decisions — owner input needed

**S4. Type annotations (M1).** `x: Int` after the name (Rust/Swift/TS,
plays well with inference) / `Int x` before (C/Java). Provisional: `name: Type`.

**S5. Comments.** `//` / `#`. Provisional: `//`.

**S7. Error propagation (M4).** Suffix `?` (terse, Rust-familiar) / prefix
`try` (reads in order: `try parse(x)`) / explicit `match` only (maximal
clarity, maximal boilerplate). Provisional: **`?` suffix** — e.g.
`parse_int(raw)?`. Prefix `try` recognized only to emit a teaching error
(see S14).

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

**S26. Compile-time execution (comptime) — DEFERRED, not open for work.**
Zig-style `comptime`: run a subset of Lex at compile time for constants,
specialized functions, and unrolled loops — evaluated in sema, lowered to
plain Rust by codegen (see architecture R1/R2). Postponed until M5+
data exists to motivate generics; Tier 2 per philosophy C1. When
revisiting, decide against Rust traits as the user-facing model. No
syntax ratified; do not implement until owner promotes from deferred.

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
| 2026-06-11 | S6  | semicolons required after every statement | owner |
| 2026-06-11 | S12 | `fn main()`, no `pub` required    | owner |
| 2026-06-11 | S19 | `while` + `for i in <range>` loops | owner |
| 2026-06-11 | S20 | minimal escapes; `{{` `}}` literal braces | owner |
| 2026-06-11 | S21 | Float always prints a decimal part | owner |
| 2026-06-11 | S22 | `1..10` is inclusive (1 through 10) | owner |
| 2026-06-11 | S23 | `break` + `continue`              | owner |
| 2026-06-11 | S24 | `switch` with condition arms (not `match`) | owner |
| 2026-06-11 | S25 | comparison distribution: `x == 1 || 2` | owner |
