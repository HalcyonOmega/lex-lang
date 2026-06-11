# 04 — Diagnostics

Error messages are the language's user interface. They are designed, not
written; every change is reviewed against this file and pinned by a
snapshot in tests/ui/.

## The contract

Every diagnostic has four parts:

- **code** — stable ID (`E0102`). Never reuse or renumber.
- **what** — one line, plain language, names the thing in backticks.
- **why** — the rule behind the error, so the user learns the model.
- **fix** — a concrete next step, copy-pasteable when possible.

## Exact render format (pinned by snapshots)

```
error[E0102]: nothing named `pirnt` exists here
  --> tests/ui/unknown_function.lex:2:5
    |
  2 |     pirnt("hi")
    |     ^^^^^
 why: only functions that have been defined (or built in, like `print`) can be called
 fix: did you mean `print`?
```

Diagnostics without a span (e.g. E0101) omit the location/source block.
Multiple diagnostics are separated by one blank line. sema reports all
problems at once; lexer/parser are fail-fast until M1 error recovery.

Lint warnings use the same shape with `warning[L02xx]:` instead of
`error[E02xx]:`. Lints do not block compilation; the driver prints them
before continuing.

## Voice rules

- Plain words. Banned: *token, expression, statement, identifier, parse,
  syntax error, illegal, invalid, lifetime, borrow checker*.
  Say: "the name `x`", "a piece of quoted text", "a number".
- Describe what the user wrote, not compiler internals.
- Ownership errors (M2) use the human framing: *while something is being
  changed, nobody else may be looking at it.*
- Staged features name their milestone and give today's workaround
  (see E0004/E0005). A future feature must never die as a generic error.
- Typos get suggestions (edit distance ≤ 2): "did you mean `print`?"
- Fixes are imperative and specific: "add a closing `\"`", never
  "consider revising".

## Error code registry

| Code  | Stage | Meaning                                  |
|-------|-------|------------------------------------------|
| E0001 | lex   | character means nothing here              |
| E0002 | lex   | unterminated text literal                 |
| E0003 | parse | expected X, found Y                       |
| E0004 | parse | staged: parameters arrive in M2           |
| E0005 | parse | staged: variables arrive in M1            |
| E0006 | parse | *reserved*                                |
| E0007 | lex   | integer too large for 64 bits             |
| E0101 | sema  | no `main` function                        |
| E0102 | sema  | unknown function (with suggestion)        |
| E0103 | sema  | `print` arity                             |
| E0104 | sema  | user function called with arguments       |
| E0105 | sema  | duplicate function definition             |
| E0106 | sema  | redefining a built-in                     |
| E0201 | sema  | `take` required; value can't be copied    |
| E0202 | sema  | `mut` required at call site               |
| E0203 | sema  | `take` on a non-consuming parameter       |
| E0206 | sema  | `view` on invalid return type             |
| E0207 | sema  | multiple unlabeled `ref` fields           |
| E0208 | sema  | `*` outside `unsafe`                      |
| L0201 | sema  | implicit `.clone()` at call site (lint)   |
| L0202 | sema  | auto-clone `Shared` inside loop (lint)    |

## Process for a new diagnostic

1. Claim the next code here. 2. Write what/why/fix per the voice rules.
3. Add a tests/ui fixture + snapshot. 4. Ship. A diagnostic without a
snapshot test does not exist (invariant I4).
