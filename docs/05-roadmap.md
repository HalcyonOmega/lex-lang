# 05 — Roadmap

Each milestone is done when its exit criteria pass as tests. Examples are
the executable spec: a milestone ships with new examples/ programs and new
tests/ui fixtures, all green.

## M0 — Walking skeleton  *(done; verified 2026-06-11)*

Hello world end-to-end: lex → parse → sema → emit Rust → rustc → run.
Diagnostics framework, ui snapshot harness, golden harness, ICE policy.
**Exit:** `cargo test` green; `lex run examples/01_hello.lex` prints. ✓

## M1 — Values and expressions  *(done; verified 2026-06-11)*

Bindings (S2: `val`/`var`), Int/Float/Bool/String (S11), arithmetic + comparison,
compound assignment (S17: `+=` `-=` `*=` `/=` `%=` `&=` `|=` `^=` `<<=`
`>>=`),
string interpolation (S8), escape sequences + `{{`/`}}` literal braces (S20),
multi-argument calls, `if`/`else`, `while` + `for i in <range>` loops
(S19; inclusive ranges S22; `break`/`continue` S23), `switch` with
condition arms (S24), Float display rule (S21),
local type inference (annotations optional, S4).
Compiler work: error recovery (multiple parse errors per run),
unicode-aware caret columns, E0005 retires. Teaching errors for familiar
foreign spellings (S14): recognize `and`/`or`/`not`, `try`, `let`/`let mut`,
`func`/`def`, `println`, `set`, `Text`, `use`, `match` and point to the
canonical Lex form (E0008–E0016). Comparison distribution in `&&`/`||`
chains (S25). No autocorrect yet — that's M6/LSP.
**Exit:** examples 03–07 (fizzbuzz-class programs + switch) run; ui suite
covers every new error; type errors name both types in plain words. ✓

## M2 — Ownership v1  ★ the crown jewel

Moves, implicit copy for scalars, explicit `.clone()`, parameter access
keywords (S10: `read`/`write`/`take`), the full ownership checker in
sema, E02xx diagnostics written to docs/04 voice rules. References cannot
be stored or returned — therefore no lifetime syntax exists.
**Exit:** an example that *fails* ownership exists for every E02xx code,
each with a snapshot whose fix line compiles when applied; golden tests
prove rustc never rejects what sema passes (the verifier earning its keep).
This milestone is where priorities 1 and 2 must both hold; budget 2–3×
the effort of any other milestone.

## M3 — Data

Structs, enums (sum types), `match` with exhaustiveness ("you forgot the
`Circle` case"), methods. No inheritance, ever (non-goal).
**Exit:** a shapes/state-machine example; exhaustiveness errors list the
missing cases verbatim.

## M4 — Errors as values

`Result`-style errors, propagation syntax (S7), `panic` for bugs.
No exceptions, no null.
**Exit:** a file-parsing example showing the happy path staying clean.

## M5 — Collections & one string story

`List`, `Map` (bridging Rust's Vec/HashMap internally), iteration,
slicing without exposing references (indices/handles). Exactly one
string type.
**Exit:** wordcount example; out-of-bounds and iterator-invalidation
mistakes produce great errors, not Rust concepts.

## M6 — Tooling

`lex fmt` (one true style, zero config), `lex test`, `lex new`. Multi-file imports (S16: `import "path" as alias;`) and visibility (S18:
`pub` exports, private by default). An LSP
server whose first job is autocorrect: rewrite recognized foreign
spellings (`and` → `&&`, `try` → `?`, `let` → `val`, `set` → `val`,
`println` → `print`,
`Text` → `String`,
etc.) to canonical
Lex on save, completing the
S14 story (error-first in M1, autocorrect here). A `lex build --small`
profile (`opt-level="z"`, full LTO; panic stance per S15). Single binary,
no config files (philosophy: minimal configuration).
**Exit:** fmt is idempotent on all examples; autocorrect turns a pasted
C-style snippet into canonical Lex; `--small` produces a measurably
smaller binary than the default; a new project runs in two commands.

## M7 — Rust FFI (interop tier)

`extern` blocks for calling vetted Rust functions across an owned/copied
boundary (no borrowed returns). This is C2's resolution: interop without
importing Rust's type system.
**Exit:** an example calling a real Rust crate function.

## Deferred indefinitely (owner can promote)

Async, user macros, traits/generics, threads & channels, package manager,
self-hosting, debugger source maps.

**Comptime (Zig-style compile-time execution)** — deferred to Tier 2,
post-M5. Revisit once structs, collections, and real programs show
whether we need generics at all, and if so whether Lex should specialize
via a `comptime` interpreter in sema (not rustc `const fn`). See
docs/02-syntax-decisions.md **S26** for the placeholder. Owner can
promote when motivated; no implementation until then.
