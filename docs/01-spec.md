# 01 — Language Spec (living document)

Everything here is provisional until ratified in docs/02-syntax-decisions.md.
The examples/ directory is the executable form of this spec: if the spec and
a passing example disagree, the spec is wrong — fix the spec.

## M0 — what exists today

### Lexical rules

- Source is UTF-8. Identifiers: a letter or `_`, then letters, digits, `_`.
- Source files use the `.lex` extension (N2).
- Line comments: `//` to end of line (S5).
- String literals: `"..."` on a single line. String interpolation via `{expr}`
  inside quotes arrives in M1 (S8). No escape sequences yet (M1).
- Numbers: decimal integers, 64-bit signed. No negatives in source yet
  (arithmetic, including unary minus, is M1).
- Whitespace separates tokens and is otherwise ignored. Statements inside
  a block end with `;` (S6, provisional). A trailing `;` before `}` is
  optional (Rust-style).

### Grammar (EBNF)

```
program  = { func } ;
func     = [ "pub" ] "fn" ident "(" ")" block ;
block    = "{" { stmt } "}" ;   // S3: curly braces
stmt     = call [ ";" ] ;
call     = ident "(" [ arg ] ")" ;
arg      = string | int ;
```

### Semantics

- A program must define `fn main` (E0101); execution starts there. `pub fn
  main` is also accepted (S18).
- Top-level items are **private by default**; prefix `pub` to export them
  to other files when imports land (S18). Within one file, all functions
  can call each other regardless of `pub`.
- `print(x)` is built in (S9); takes exactly one argument (E0103) and writes
  it to stdout followed by a newline.
- Calls resolve to user functions or built-ins; unknown names are E0102
  with a did-you-mean suggestion when an existing name is within edit
  distance 2.
- Function names are unique (E0105) and may not shadow built-ins (E0106).

### Staged errors

Features that exist in the roadmap but not the language yet fail with an
error naming the milestone (E0004 parameters → M2, E0005 variables → M1).
A future feature must never die as a generic syntax error.

## M1 preview — values and expressions (designed, not yet built)

- Bindings: `val` for immutable, `var` for mutable (S2).
- Arithmetic: `+` `-` `*` `/` on `Int` and `Float`; `%` `&` `|` `^` `<<`
  `>>` on `Int`.
- Compound assignment (S17): `+=` `-=` `*=` `/=` on `Int` and `Float`;
  `%=`, `&=`, `|=`, `^=`, `<<=`, `>>=` on `Int`; all on mutable bindings
  and `write` parameters.
- Comparison and logic: S13 operators.

## M2 — ownership transpiler (partial; growing)

Borrow-checker mechanics live in the transpiler; tier-1 users never write
`&`, `&mut`, `*`, or lifetime parameters.

| You write              | It means                          | Compiles to Rust |
|------------------------|-----------------------------------|------------------|
| `fn f(x: T)`           | shared read (default)             | `x: &T`          |
| `fn f(mut x: T)`       | mutable borrow                    | `x: &mut T`      |
| `fn f(take x: T)`      | move; caller must write `take`    | `x: T`           |
| `fn f() -> view T`     | borrow return (elided lifetime)   | `-> &T`          |
| `ref field: T` (tier 2)| stored reference in a struct      | `field: &'a T`   |

Call-site rules: `mut` and `take` must match the parameter; omitting `take`
on a clonable type inserts `.clone()` with lint **L0201**; on a
non-clonable type → **E0201**. Omitting `mut` on a mutable parameter →
**E0202**. `*` outside `unsafe` → **E0208**.

`const NAME = value` always looks the same; the transpiler emits Rust
`const` or `static` when the address is taken or the type needs it.

Aliasing rule, stated for humans: *while something is being changed,
nobody else may be looking at it.* All E02xx diagnostics (reserved range)
must explain violations in those terms, with a what/why/fix.

## Deliberately absent

See non-goals in docs/00-philosophy.md. The parser should produce staged
or guiding errors for the ones users will reach for (e.g. `and` → teaching
error naming `&&`, per S14).
