---
paths:
  - "playground/**/*.ts"
  - "playground/**/*.tsx"
---

# TypeScript / Playground Conventions

- **No `any` types. Ever.** Use interfaces, generics, discriminated unions.
- **pnpm** for package management. Never npm/yarn. Never edit lock files — use `pnpm add`.
- React components **≤150 lines** — extract hooks, child components, utilities aggressively.
- Push state down, lift events up. Prefer composition over prop drilling.
- No unnecessary `useEffect` — prefer derived state or event handlers.
- `useMemo`/`useCallback` only where measurably needed.

## WASM boundary

- All WASM calls go through `wasm/docmux.ts`. Components never import wasm bindings directly.
- WASM calls can panic — ensure try/catch at the boundary.
- WASM conversion should be debounced.

## State & styling

- Dexie (IndexedDB) for persistence, React context for UI state.
- Tailwind CSS 4 + shadcn/ui patterns. Use `cn()` for conditional classes.
- Dark mode must work (test both themes).
- Feature-based organization under `components/`: `editor/`, `file-tree/`, `output-tabs/`, `ui/`.
