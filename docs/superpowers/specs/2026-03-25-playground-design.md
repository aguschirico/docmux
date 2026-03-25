# docmux Playground — Design Spec

> Date: 2026-03-25
> Status: Approved
> Scope: `playground/` directory (Vite + React + TypeScript app)

## Goal

Build an interactive playground app to test and demo docmux WASM conversions. Split-pane editor with file tree, Monaco editor, and tabbed output panel. Includes a debug mode (AST viewer, diagnostics) toggled via tabs.

This iteration covers scaffolding, VFS, editor, and UI shell. WASM integration is deferred to a future iteration (part of the docmux roadmap).

## Architecture

```
playground/
├── src/
│   ├── main.tsx
│   ├── index.css             (Tailwind v4 entry: @import "tailwindcss")
│   ├── App.tsx               (layout shell: header + 3-pane resizable)
│   ├── vfs/
│   │   ├── types.ts          (VfsFile, VfsWorkspace interfaces)
│   │   ├── db.ts             (Dexie DB schema + CRUD operations)
│   │   └── import.ts         (File System Access API → VFS import)
│   ├── contexts/
│   │   └── workspace-context.tsx  (React Context for workspace/file state)
│   ├── components/
│   │   ├── FileTree.tsx      (workspace file tree sidebar)
│   │   ├── Editor.tsx        (Monaco editor wrapper)
│   │   ├── OutputTabs.tsx    (tab container: Preview | Source | AST | Diagnostics)
│   │   └── Header.tsx        (app header with workspace selector + import)
│   └── lib/
│       ├── formats.ts        (extension → format name, Monaco language mapping)
│       └── debounce.ts       (simple debounce utility)
├── package.json
├── vite.config.ts
├── tsconfig.json
├── tsconfig.app.json         (app source compiler options)
├── tsconfig.node.json        (vite.config.ts compiler options)
├── index.html
└── components.json           (shadcn v2 config, Tailwind v4)
```

## Layout

Three horizontal resizable panels using shadcn `ResizablePanelGroup`:

```
┌──────────┬─────────────────────┬─────────────────────┐
│ FileTree │      Editor         │    Output Tabs       │
│  (20%)   │      (40%)          │      (40%)           │
│          │                     │ [Preview|Source|AST|  │
│          │                     │  Diagnostics]         │
│          │                     │                       │
│          │                     │  (placeholder until   │
│          │                     │   WASM connected)     │
└──────────┴─────────────────────┴─────────────────────┘
```

Header bar above: app name, workspace selector dropdown, "Import Folder" button.

## State Management

A single React Context (`WorkspaceContext`) holds the app-level state:

```ts
interface WorkspaceState {
  activeWorkspaceId: number | null;
  activeFileId: number | null;
  setActiveWorkspaceId: (id: number | null) => void;
  setActiveFileId: (id: number | null) => void;
}
```

- `Header` writes `activeWorkspaceId` (workspace selector)
- `FileTree` reads `activeWorkspaceId`, writes `activeFileId` (file click)
- `Editor` reads `activeFileId`, loads content via `useLiveQuery` from Dexie
- `OutputTabs` reads `activeFileId` (future: triggers conversion)

All VFS data (file list, workspace list) is accessed via `dexie-react-hooks` `useLiveQuery`, which auto-updates when IndexedDB changes. No need to duplicate DB state in React state.

## Virtual Filesystem (VFS)

### Storage: Dexie (IndexedDB)

Two tables:

```
workspaces:
  id:          number (auto-increment, primary key)
  name:        string
  createdAt:   Date

files:
  id:          number (auto-increment, primary key)
  workspaceId: number (indexed)
  path:        string (relative, e.g. "docs/intro.md")
  content:     string
  updatedAt:   Date

  Compound unique index: [workspaceId+path]
```

- `path` is relative to the workspace root
- Format is detected from file extension at read time (not stored)
- **Folders are implicit** — derived from file paths by splitting on `/`. There are no explicit folder entries. Creating a "folder" in the UI creates a file inside it (prompts for filename). Renaming/deleting a folder renames/deletes all files with that path prefix.
- Operations: create/rename/delete files; create folder (= create first file in it); rename/delete folder (= batch rename/delete by path prefix)

### Error Handling

VFS operations can fail (IndexedDB quota, private browsing restrictions, Dexie errors). Errors are surfaced via shadcn `Sonner` toasts at the bottom of the screen.

### Import Folder

"Import Folder" button uses `window.showDirectoryPicker()` (File System Access API). Recursively reads all text files from the directory handle and inserts them into the VFS as a new workspace named after the folder.

Fallback for browsers without File System Access API: `<input type="file" webkitdirectory>`.

Requires `@anthropic-ai/wicg-file-system-access` types or a custom `src/types/file-system-access.d.ts` declaration file.

### Default Workspace

On first launch (empty DB), create a workspace "Example" with a single `example.md` containing a sample document that exercises headings, math, tables, code blocks, and lists.

## Components

### FileTree

- Hierarchical tree built by splitting file `path` on `/`
- Context menu (right-click): New File, New Folder, Rename, Delete
- File type icons via lucide-react (by extension: `.md`, `.tex`, `.typ`, generic)
- Click file → open in editor (set as active file)
- Active file highlighted
- Built with shadcn primitives (ScrollArea, ContextMenu, Button), no external tree library

### Editor

- `@monaco-editor/react` with `vs-dark` theme
- Language detection by extension:
  - `.md` → `markdown`
  - `.tex` → `latex`
  - `.typ` → `typescript` (closest available Monaco language)
  - `.html` → `html`
  - `.json` → `json`
  - fallback → `plaintext`
- `onChange` → debounced save to VFS (500ms) via hand-rolled `debounce` utility in `lib/debounce.ts`
- Shows empty state when no file selected

### OutputTabs

Four tabs via shadcn `Tabs`:
1. **Preview** — will render converted HTML (placeholder for now)
2. **Source** — will show raw converted output (placeholder for now)
3. **AST** — will show parsed AST as JSON tree (placeholder for now)
4. **Diagnostics** — will show parse errors/warnings (placeholder for now)

Each tab shows centered placeholder text: "Connect docmux WASM to enable conversion"

### Header

- App name: "docmux playground"
- Workspace selector: shadcn Select/dropdown listing all workspaces
- "Import Folder" button
- "New Workspace" button

## Styling

- Dark mode by default (zinc/neutral palette)
- Geist Sans for UI, Geist Mono for code/editor areas
- Subtle borders between resizable panels
- shadcn v2 with Tailwind CSS v4 (uses `@import "tailwindcss"` in `index.css`, CSS-based config)

## Dependencies

### Runtime
- react, react-dom
- @monaco-editor/react
- dexie, dexie-react-hooks
- tailwindcss, @tailwindcss/vite
- shadcn/ui (v2, Tailwind v4 compatible) components: tabs, resizable, context-menu, button, input, dialog, scroll-area, select, separator, dropdown-menu, sonner
- lucide-react
- sonner (toast notifications for errors)
- geist (font)

### Dev
- vite
- typescript
- @types/react, @types/react-dom

## Data Flow

1. User selects file in FileTree → context updates `activeFileId`
2. `activeFileId` change → `useLiveQuery` loads file content from Dexie → set in Monaco
3. Monaco `onChange` → debounce 500ms → save content back to Dexie
4. (Future) On content change → `convert(content, from, to)` via WASM → feed result to active output tab

## Repo Hygiene

- Add `playground/node_modules/` and `playground/dist/` to root `.gitignore`
- The `playground/` directory is independent of the Cargo workspace (no changes to root `Cargo.toml`)

## Deferred (Future WASM Integration)

- `wasm/docmux.ts` wrapper (init + convert + parse)
- Real HTML preview in Preview tab
- Source output in Source tab
- AST JSON in AST tab (expects `parse(input: string, format: string) → string` returning JSON-serialized `Document`)
- Error/warning display in Diagnostics tab
- Adding `parse()` function to `crates/docmux-wasm/src/lib.rs`

### LaTeX Preview via WASM (future)

For the Preview tab when viewing `.tex` files, compile LaTeX to PDF in-browser using a WASM-based TeX engine (e.g. texlive-wasm or SwiftLaTeX). Strategy:
- **On-demand loading** — the LaTeX WASM engine (~30MB+) is only fetched when the user first requests PDF preview for a `.tex` file
- **Cache** — once downloaded, cache the WASM binary in IndexedDB or Cache API so subsequent visits are instant
- **Pipeline** — docmux converts source → LaTeX, then the TeX engine compiles LaTeX → PDF, rendered via `<iframe>` or pdf.js

## File Summary

| Action | Path |
|--------|------|
| Create | `playground/package.json` |
| Create | `playground/vite.config.ts` |
| Create | `playground/tsconfig.json` |
| Create | `playground/tsconfig.app.json` |
| Create | `playground/tsconfig.node.json` |
| Create | `playground/index.html` |
| Create | `playground/components.json` |
| Create | `playground/src/main.tsx` |
| Create | `playground/src/index.css` |
| Create | `playground/src/App.tsx` |
| Create | `playground/src/vfs/types.ts` |
| Create | `playground/src/vfs/db.ts` |
| Create | `playground/src/vfs/import.ts` |
| Create | `playground/src/contexts/workspace-context.tsx` |
| Create | `playground/src/components/FileTree.tsx` |
| Create | `playground/src/components/Editor.tsx` |
| Create | `playground/src/components/OutputTabs.tsx` |
| Create | `playground/src/components/Header.tsx` |
| Create | `playground/src/lib/formats.ts` |
| Create | `playground/src/lib/debounce.ts` |
| Create | `playground/src/types/file-system-access.d.ts` |
| Modify | `.gitignore` (add playground/node_modules/, playground/dist/) |
