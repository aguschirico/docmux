## QUALITY REVIEW REQUIRED

You have uncommitted code changes. Before completing, review ALL your changes against this checklist. Run `git diff HEAD` to see everything you changed, then verify:

### DRY
- No repeated logic — if you see the same pattern 2+ times, extract it (helper function, shared module, custom hook).
- No copy-paste with minor variations — generalize.

### Dead Code
- No unused functions, variables, imports, types, or struct fields.
- No commented-out code. Delete it — git has history.
- No unreachable branches or impossible match arms.

### Simplicity
- Is this the simplest solution that works? Could it be done with less code?
- No premature abstractions — don't build for hypothetical future needs.
- No over-engineering: 3 lines of clear code > 1 clever line.

### Maintainability
- Functions do one thing. Rust functions ≤40 lines, React components ≤150 lines.
- Naming is clear and self-documenting. No abbreviations that need mental decoding.
- Data flows are obvious — no hidden side effects, no spooky action at a distance.

### Language Best Practices
**Rust:**
- Idiomatic: iterators over index loops, `?` over match-then-return, `impl Into<String>` for params.
- No `unwrap()`/`expect()` in lib code. Use `?` and descriptive errors.
- No unnecessary `.clone()` — borrow if you can.
- Match arms are exhaustive — new AST nodes handled in all writers.

**TypeScript/React:**
- No `any`. Ever. Use proper types, generics, discriminated unions.
- No unnecessary `useEffect` — prefer derived state or event handlers.
- State management is minimal — don't store derivable data.
- WASM calls go through `wasm/docmux.ts`, never direct imports.

### Completeness
- Does the code fully implement what was asked? No partial implementations left behind.
- Edge cases handled: empty input, malformed data, unicode, large input.
- Error messages are descriptive and actionable.

### Tests
- New functionality has tests. No exceptions.
- Tests cover the happy path AND at least one error/edge case.
- Golden file tests updated if output format changed.

If you find ANY issue, fix it now. Then verify again. Only complete when everything passes.
