# docmux Playground Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a playground app (Vite + React + Monaco + shadcn/ui) with a virtual filesystem, file tree, editor, and placeholder output tabs for testing docmux WASM conversions.

**Architecture:** Three-pane resizable layout (file tree | editor | output tabs) backed by a Dexie/IndexedDB virtual filesystem. React Context holds active workspace/file IDs. All VFS data accessed reactively via `useLiveQuery`.

**Tech Stack:** pnpm, Vite, React 19, TypeScript, Tailwind CSS v4, shadcn/ui v2, Monaco Editor, Dexie 4, Geist font, lucide-react

**Spec:** `docs/superpowers/specs/2026-03-25-playground-design.md`

---

## File Structure

```
playground/
├── index.html
├── package.json
├── pnpm-lock.yaml
├── vite.config.ts
├── tsconfig.json
├── tsconfig.app.json
├── tsconfig.node.json
├── components.json
├── src/
│   ├── main.tsx
│   ├── index.css
│   ├── App.tsx
│   ├── types/
│   │   └── file-system-access.d.ts
│   ├── vfs/
│   │   ├── types.ts
│   │   ├── db.ts
│   │   └── import.ts
│   ├── contexts/
│   │   └── workspace-context.tsx
│   ├── components/
│   │   ├── FileTree.tsx
│   │   ├── Editor.tsx
│   │   ├── OutputTabs.tsx
│   │   └── Header.tsx
│   └── lib/
│       ├── utils.ts              (shadcn cn() utility)
│       ├── formats.ts
│       └── debounce.ts
```

---

### Task 1: Scaffold Vite + React + TypeScript project

**Files:**
- Create: `playground/package.json`
- Create: `playground/index.html`
- Create: `playground/vite.config.ts`
- Create: `playground/tsconfig.json`
- Create: `playground/tsconfig.app.json`
- Create: `playground/tsconfig.node.json`
- Create: `playground/src/main.tsx`
- Create: `playground/src/index.css`
- Create: `playground/src/App.tsx`
- Modify: `.gitignore`

- [ ] **Step 1: Create `playground/` directory and scaffold with Vite**

Run from repo root:

```bash
pnpm create vite playground --template react-ts
```

This generates the project skeleton. Then clean up default files we don't need:

```bash
cd playground
rm -f src/App.css src/assets/react.svg public/vite.svg src/index.css
```

- [ ] **Step 2: Install Tailwind CSS v4 with Vite plugin**

```bash
cd playground && pnpm add tailwindcss @tailwindcss/vite
```

- [ ] **Step 3: Configure Vite with Tailwind plugin**

Replace `playground/vite.config.ts`:

```ts
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "path";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
});
```

- [ ] **Step 4: Create `playground/src/index.css`**

```css
@import "tailwindcss";
@import "geist/font/geist-sans.css";
@import "geist/font/geist-mono.css";

:root {
  font-family: "Geist Sans", sans-serif;
}

code, pre, .font-mono {
  font-family: "Geist Mono", monospace;
}

html {
  color-scheme: dark;
}
```

- [ ] **Step 5: Create minimal `playground/src/App.tsx`**

```tsx
export default function App() {
  return (
    <div className="h-screen bg-zinc-950 text-zinc-100">
      <p className="p-4">docmux playground</p>
    </div>
  );
}
```

- [ ] **Step 6: Update `playground/src/main.tsx`**

```tsx
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "./index.css";
import App from "./App";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
```

- [ ] **Step 7: Update `playground/tsconfig.app.json` with path alias**

Ensure it has:

```json
{
  "compilerOptions": {
    "target": "ES2020",
    "useDefineForClassFields": true,
    "lib": ["ES2020", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "isolatedModules": true,
    "moduleDetection": "force",
    "noEmit": true,
    "jsx": "react-jsx",
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true,
    "noUncheckedIndexedAccess": true,
    "baseUrl": ".",
    "paths": {
      "@/*": ["./src/*"]
    }
  },
  "include": ["src"]
}
```

- [ ] **Step 8: Add playground to root `.gitignore`**

Append to `.gitignore` (note: `node_modules/` is already covered globally):

```
playground/dist/
```

- [ ] **Step 9: Install Geist font**

```bash
cd playground && pnpm add geist
```

- [ ] **Step 10: Verify dev server starts**

```bash
cd playground && pnpm dev
```

Expected: Vite dev server starts, browser shows "docmux playground" on dark background.

- [ ] **Step 11: Commit**

```bash
git add playground/ .gitignore
git commit -m "feat(playground): scaffold Vite + React + TypeScript + Tailwind v4"
```

---

### Task 2: Initialize shadcn/ui

**Files:**
- Create: `playground/components.json`
- Create: `playground/src/lib/utils.ts`

- [ ] **Step 1: Initialize shadcn**

```bash
cd playground && pnpm dlx shadcn@latest init
```

When prompted:
- Style: New York
- Base color: Zinc
- CSS variables: yes

This creates `components.json` and `src/lib/utils.ts` (with the `cn()` utility).

- [ ] **Step 2: Install required shadcn components**

```bash
cd playground && pnpm dlx shadcn@latest add tabs resizable context-menu button input dialog scroll-area select separator dropdown-menu sonner
```

- [ ] **Step 3: Verify build still works**

```bash
cd playground && pnpm build
```

Expected: Build succeeds with no errors.

- [ ] **Step 4: Commit**

```bash
git add playground/
git commit -m "feat(playground): initialize shadcn/ui v2 with required components"
```

---

### Task 3: VFS types and Dexie database

**Files:**
- Create: `playground/src/vfs/types.ts`
- Create: `playground/src/vfs/db.ts`

- [ ] **Step 1: Install Dexie**

```bash
cd playground && pnpm add dexie dexie-react-hooks
```

- [ ] **Step 2: Create `playground/src/vfs/types.ts`**

```ts
export interface VfsWorkspace {
  id?: number;
  name: string;
  createdAt: Date;
}

export interface VfsFile {
  id?: number;
  workspaceId: number;
  path: string;
  content: string;
  updatedAt: Date;
}
```

- [ ] **Step 3: Create `playground/src/vfs/db.ts`**

```ts
import Dexie, { type EntityTable } from "dexie";
import type { VfsWorkspace, VfsFile } from "./types";

const EXAMPLE_MD = `# Welcome to docmux

This is an **example document** to get you started.

## Math

Inline: $E = mc^2$

Display:

$$
\\int_0^\\infty e^{-x^2} dx = \\frac{\\sqrt{\\pi}}{2}
$$

## Table

| Format   | Reader | Writer |
|----------|--------|--------|
| Markdown | ✅     | —      |
| HTML     | —      | ✅     |
| LaTeX    | ✅     | ✅     |
| Typst    | ✅     | —      |

## Code

\`\`\`rust
fn main() {
    println!("Hello from docmux!");
}
\`\`\`

## Lists

- Item one
- Item two
  - Nested item
- Item three

1. First
2. Second
3. Third
`;

class DocmuxVfsDb extends Dexie {
  workspaces!: EntityTable<VfsWorkspace, "id">;
  files!: EntityTable<VfsFile, "id">;

  constructor() {
    super("docmux-vfs");
    this.version(1).stores({
      workspaces: "++id, name",
      files: "++id, workspaceId, path, [workspaceId+path]",
    });
  }
}

export const db = new DocmuxVfsDb();

export async function ensureDefaultWorkspace(): Promise<void> {
  const count = await db.workspaces.count();
  if (count > 0) return;

  const wsId = await db.workspaces.add({
    name: "Example",
    createdAt: new Date(),
  });

  await db.files.add({
    workspaceId: wsId,
    path: "example.md",
    content: EXAMPLE_MD,
    updatedAt: new Date(),
  });
}

export async function createWorkspace(name: string): Promise<number> {
  return db.workspaces.add({
    name,
    createdAt: new Date(),
  });
}

export async function createFile(
  workspaceId: number,
  path: string,
  content = "",
): Promise<number> {
  return db.files.add({
    workspaceId,
    path,
    content,
    updatedAt: new Date(),
  });
}

export async function updateFileContent(
  fileId: number,
  content: string,
): Promise<void> {
  await db.files.update(fileId, {
    content,
    updatedAt: new Date(),
  });
}

export async function renameFile(
  fileId: number,
  newPath: string,
): Promise<void> {
  await db.files.update(fileId, {
    path: newPath,
    updatedAt: new Date(),
  });
}

export async function deleteFile(fileId: number): Promise<void> {
  await db.files.delete(fileId);
}

export async function renameFolder(
  workspaceId: number,
  oldPrefix: string,
  newPrefix: string,
): Promise<void> {
  const files = await db.files
    .where("workspaceId")
    .equals(workspaceId)
    .filter((f) => f.path.startsWith(oldPrefix + "/"))
    .toArray();

  await db.transaction("rw", db.files, async () => {
    for (const file of files) {
      await db.files.update(file.id!, {
        path: newPrefix + file.path.slice(oldPrefix.length),
        updatedAt: new Date(),
      });
    }
  });
}

export async function deleteFolder(
  workspaceId: number,
  prefix: string,
): Promise<void> {
  const files = await db.files
    .where("workspaceId")
    .equals(workspaceId)
    .filter((f) => f.path.startsWith(prefix + "/"))
    .toArray();

  await db.files.bulkDelete(files.map((f) => f.id!));
}

export async function deleteWorkspace(workspaceId: number): Promise<void> {
  await db.transaction("rw", [db.workspaces, db.files], async () => {
    await db.files.where("workspaceId").equals(workspaceId).delete();
    await db.workspaces.delete(workspaceId);
  });
}
```

- [ ] **Step 4: Verify build**

```bash
cd playground && pnpm build
```

Expected: Build succeeds.

- [ ] **Step 5: Commit**

```bash
git add playground/src/vfs/
git commit -m "feat(playground): VFS types and Dexie database with CRUD operations"
```

---

### Task 4: Workspace context and format utilities

**Files:**
- Create: `playground/src/contexts/workspace-context.tsx`
- Create: `playground/src/lib/formats.ts`
- Create: `playground/src/lib/debounce.ts`

- [ ] **Step 1: Create `playground/src/contexts/workspace-context.tsx`**

```tsx
import { createContext, useContext, useState, type ReactNode } from "react";

interface WorkspaceState {
  activeWorkspaceId: number | null;
  activeFileId: number | null;
  setActiveWorkspaceId: (id: number | null) => void;
  setActiveFileId: (id: number | null) => void;
}

const WorkspaceContext = createContext<WorkspaceState | null>(null);

export function WorkspaceProvider({ children }: { children: ReactNode }) {
  const [activeWorkspaceId, setActiveWorkspaceId] = useState<number | null>(
    null,
  );
  const [activeFileId, setActiveFileId] = useState<number | null>(null);

  return (
    <WorkspaceContext.Provider
      value={{
        activeWorkspaceId,
        activeFileId,
        setActiveWorkspaceId,
        setActiveFileId,
      }}
    >
      {children}
    </WorkspaceContext.Provider>
  );
}

export function useWorkspace(): WorkspaceState {
  const ctx = useContext(WorkspaceContext);
  if (!ctx) {
    throw new Error("useWorkspace must be used within WorkspaceProvider");
  }
  return ctx;
}
```

- [ ] **Step 2: Create `playground/src/lib/formats.ts`**

```ts
const EXTENSION_TO_FORMAT: Record<string, string> = {
  md: "markdown",
  markdown: "markdown",
  tex: "latex",
  latex: "latex",
  typ: "typst",
  html: "html",
  htm: "html",
  json: "json",
  xml: "xml",
  yaml: "yaml",
  yml: "yaml",
  bib: "bibtex",
};

const EXTENSION_TO_MONACO: Record<string, string> = {
  md: "markdown",
  markdown: "markdown",
  tex: "latex",
  latex: "latex",
  typ: "typescript",
  html: "html",
  htm: "html",
  json: "json",
  xml: "xml",
  yaml: "yaml",
  yml: "yaml",
  css: "css",
  js: "javascript",
  ts: "typescript",
  bib: "plaintext",
};

export function getExtension(path: string): string {
  const dot = path.lastIndexOf(".");
  return dot >= 0 ? path.slice(dot + 1).toLowerCase() : "";
}

export function getFormat(path: string): string {
  return EXTENSION_TO_FORMAT[getExtension(path)] ?? "plaintext";
}

export function getMonacoLanguage(path: string): string {
  return EXTENSION_TO_MONACO[getExtension(path)] ?? "plaintext";
}
```

- [ ] **Step 3: Create `playground/src/lib/debounce.ts`**

```ts
export function debounce<T extends (...args: Parameters<T>) => void>(
  fn: T,
  ms: number,
): (...args: Parameters<T>) => void {
  let timer: ReturnType<typeof setTimeout>;
  return (...args: Parameters<T>) => {
    clearTimeout(timer);
    timer = setTimeout(() => fn(...args), ms);
  };
}
```

- [ ] **Step 4: Verify build**

```bash
cd playground && pnpm build
```

Expected: Build succeeds.

- [ ] **Step 5: Commit**

```bash
git add playground/src/contexts/ playground/src/lib/
git commit -m "feat(playground): workspace context, format utils, debounce"
```

---

### Task 5: Header component + folder import

**Files:**
- Create: `playground/src/components/Header.tsx`
- Create: `playground/src/vfs/import.ts`
- Create: `playground/src/types/file-system-access.d.ts`

- [ ] **Step 1: Install lucide-react**

```bash
cd playground && pnpm add lucide-react
```

- [ ] **Step 2: Create `playground/src/types/file-system-access.d.ts`**

```ts
interface FileSystemDirectoryHandle {
  kind: "directory";
  name: string;
  values(): AsyncIterableIterator<
    FileSystemDirectoryHandle | FileSystemFileHandle
  >;
}

interface FileSystemFileHandle {
  kind: "file";
  name: string;
  getFile(): Promise<File>;
}

interface Window {
  showDirectoryPicker?: () => Promise<FileSystemDirectoryHandle>;
}
```

- [ ] **Step 3: Create `playground/src/vfs/import.ts`**

```ts
import { db, createWorkspace } from "./db";

async function readDirectoryRecursive(
  handle: FileSystemDirectoryHandle,
  prefix: string,
): Promise<Array<{ path: string; content: string }>> {
  const entries: Array<{ path: string; content: string }> = [];

  for await (const entry of handle.values()) {
    const entryPath = prefix ? `${prefix}/${entry.name}` : entry.name;

    if (entry.kind === "file") {
      try {
        const file = await (entry as FileSystemFileHandle).getFile();
        const content = await file.text();
        entries.push({ path: entryPath, content });
      } catch {
        // Skip binary or unreadable files
      }
    } else if (entry.kind === "directory") {
      const children = await readDirectoryRecursive(
        entry as FileSystemDirectoryHandle,
        entryPath,
      );
      entries.push(...children);
    }
  }

  return entries;
}

async function importViaDirectoryPicker(): Promise<number | null> {
  if (!window.showDirectoryPicker) return null;

  const dirHandle = await window.showDirectoryPicker();
  const entries = await readDirectoryRecursive(dirHandle, "");

  if (entries.length === 0) return null;

  const wsId = await createWorkspace(dirHandle.name);
  const now = new Date();

  await db.files.bulkAdd(
    entries.map((e) => ({
      workspaceId: wsId,
      path: e.path,
      content: e.content,
      updatedAt: now,
    })),
  );

  return wsId;
}

async function importViaFileInput(): Promise<number | null> {
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.webkitdirectory = true;

    input.onchange = async () => {
      const files = input.files;
      if (!files || files.length === 0) {
        resolve(null);
        return;
      }

      // Derive workspace name from first file's path
      const firstFile = files[0];
      if (!firstFile) {
        resolve(null);
        return;
      }
      const wsName = firstFile.webkitRelativePath.split("/")[0] ?? "Imported";
      const wsId = await createWorkspace(wsName);
      const now = new Date();

      const entries: Array<{
        workspaceId: number;
        path: string;
        content: string;
        updatedAt: Date;
      }> = [];

      for (const file of Array.from(files)) {
        try {
          const content = await file.text();
          // Remove the root folder prefix from webkitRelativePath
          const parts = file.webkitRelativePath.split("/");
          const path = parts.slice(1).join("/");
          entries.push({ workspaceId: wsId, path, content, updatedAt: now });
        } catch {
          // Skip binary or unreadable files
        }
      }

      await db.files.bulkAdd(entries);
      resolve(wsId);
    };

    input.oncancel = () => resolve(null);
    input.click();
  });
}

export async function importFolder(): Promise<number | null> {
  if (window.showDirectoryPicker) {
    return importViaDirectoryPicker();
  }
  return importViaFileInput();
}
```

- [ ] **Step 4: Create `playground/src/components/Header.tsx`**

```tsx
import { useLiveQuery } from "dexie-react-hooks";
import { FolderOpen, Plus } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useWorkspace } from "@/contexts/workspace-context";
import { db, createWorkspace } from "@/vfs/db";
import { importFolder } from "@/vfs/import";
import { toast } from "sonner";

export function Header() {
  const { activeWorkspaceId, setActiveWorkspaceId, setActiveFileId } =
    useWorkspace();
  const workspaces = useLiveQuery(() => db.workspaces.toArray());

  async function handleNewWorkspace() {
    try {
      const name = `Workspace ${(workspaces?.length ?? 0) + 1}`;
      const id = await createWorkspace(name);
      setActiveWorkspaceId(id);
      setActiveFileId(null);
    } catch (err) {
      toast.error(`Failed to create workspace: ${err}`);
    }
  }

  async function handleImportFolder() {
    try {
      const wsId = await importFolder();
      if (wsId !== null) {
        setActiveWorkspaceId(wsId);
        setActiveFileId(null);
      }
    } catch (err) {
      toast.error(`Failed to import folder: ${err}`);
    }
  }

  return (
    <header className="flex h-12 items-center gap-3 border-b border-zinc-800 bg-zinc-950 px-4">
      <span className="text-sm font-semibold tracking-tight text-zinc-100">
        docmux playground
      </span>

      <Separator />

      <Select
        value={activeWorkspaceId?.toString() ?? ""}
        onValueChange={(val) => {
          setActiveWorkspaceId(Number(val));
          setActiveFileId(null);
        }}
      >
        <SelectTrigger className="h-8 w-48 text-xs">
          <SelectValue placeholder="Select workspace" />
        </SelectTrigger>
        <SelectContent>
          {workspaces?.map((ws) => (
            <SelectItem key={ws.id} value={ws.id!.toString()}>
              {ws.name}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>

      <Button
        variant="ghost"
        size="sm"
        className="h-8 gap-1.5 text-xs"
        onClick={handleImportFolder}
      >
        <FolderOpen className="h-3.5 w-3.5" />
        Import Folder
      </Button>

      <Button
        variant="ghost"
        size="sm"
        className="h-8 gap-1.5 text-xs"
        onClick={handleNewWorkspace}
      >
        <Plus className="h-3.5 w-3.5" />
        New
      </Button>
    </header>
  );
}

function Separator() {
  return <div className="h-4 w-px bg-zinc-800" />;
}
```

- [ ] **Step 5: Verify build**

```bash
cd playground && pnpm build
```

Expected: Build succeeds.

- [ ] **Step 6: Commit**

```bash
git add playground/src/components/Header.tsx playground/src/vfs/import.ts playground/src/types/
git commit -m "feat(playground): Header component with workspace selector + folder import"
```

---

### Task 6: FileTree component

**Files:**
- Create: `playground/src/components/FileTree.tsx`

- [ ] **Step 1: Create `playground/src/components/FileTree.tsx`**

```tsx
import { useMemo, useState } from "react";
import { useLiveQuery } from "dexie-react-hooks";
import {
  File,
  Folder,
  FolderOpen,
  ChevronRight,
  ChevronDown,
  FileText,
  FileCode,
} from "lucide-react";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useWorkspace } from "@/contexts/workspace-context";
import { db, createFile, renameFile, renameFolder, deleteFile, deleteFolder } from "@/vfs/db";
import { getExtension } from "@/lib/formats";
import { toast } from "sonner";

interface TreeNode {
  name: string;
  path: string;
  isFolder: boolean;
  fileId?: number;
  children: TreeNode[];
}

function buildTree(
  files: Array<{ id?: number; path: string }>,
): TreeNode[] {
  const root: TreeNode[] = [];

  for (const file of files) {
    const parts = file.path.split("/");
    let current = root;

    for (let i = 0; i < parts.length; i++) {
      const name = parts[i]!;
      const isLast = i === parts.length - 1;
      const partPath = parts.slice(0, i + 1).join("/");

      let existing = current.find((n) => n.name === name);

      if (!existing) {
        existing = {
          name,
          path: partPath,
          isFolder: !isLast,
          fileId: isLast ? file.id : undefined,
          children: [],
        };
        current.push(existing);
      }

      if (!isLast) {
        current = existing.children;
      }
    }
  }

  // Sort: folders first, then alphabetical
  function sortNodes(nodes: TreeNode[]): TreeNode[] {
    nodes.sort((a, b) => {
      if (a.isFolder !== b.isFolder) return a.isFolder ? -1 : 1;
      return a.name.localeCompare(b.name);
    });
    for (const node of nodes) {
      sortNodes(node.children);
    }
    return nodes;
  }

  return sortNodes(root);
}

function getFileIcon(name: string) {
  const ext = getExtension(name);
  switch (ext) {
    case "md":
    case "markdown":
      return <FileText className="h-4 w-4 shrink-0 text-zinc-500" />;
    case "tex":
    case "latex":
    case "typ":
      return <FileCode className="h-4 w-4 shrink-0 text-zinc-500" />;
    default:
      return <File className="h-4 w-4 shrink-0 text-zinc-500" />;
  }
}

function TreeItem({
  node,
  depth,
  activeFileId,
  onSelectFile,
  onNewFile,
  onDelete,
  onRename,
}: {
  node: TreeNode;
  depth: number;
  activeFileId: number | null;
  onSelectFile: (id: number) => void;
  onNewFile: (folderPath: string) => void;
  onNewFolder: (folderPath: string) => void;
  onDelete: (node: TreeNode) => void;
  onRename: (node: TreeNode) => void;
}) {
  const [expanded, setExpanded] = useState(depth === 0);
  const isActive = !node.isFolder && node.fileId === activeFileId;

  return (
    <>
      <ContextMenu>
        <ContextMenuTrigger asChild>
          <button
            className={`flex w-full items-center gap-1.5 rounded-sm px-2 py-1 text-left text-xs hover:bg-zinc-800 ${
              isActive ? "bg-zinc-800 text-zinc-100" : "text-zinc-400"
            }`}
            style={{ paddingLeft: `${depth * 12 + 8}px` }}
            onClick={() => {
              if (node.isFolder) {
                setExpanded(!expanded);
              } else if (node.fileId !== undefined) {
                onSelectFile(node.fileId);
              }
            }}
          >
            {node.isFolder ? (
              <>
                {expanded ? (
                  <ChevronDown className="h-3.5 w-3.5 shrink-0 text-zinc-600" />
                ) : (
                  <ChevronRight className="h-3.5 w-3.5 shrink-0 text-zinc-600" />
                )}
                {expanded ? (
                  <FolderOpen className="h-4 w-4 shrink-0 text-zinc-500" />
                ) : (
                  <Folder className="h-4 w-4 shrink-0 text-zinc-500" />
                )}
              </>
            ) : (
              <>
                <span className="w-3.5" />
                {getFileIcon(node.name)}
              </>
            )}
            <span className="truncate">{node.name}</span>
          </button>
        </ContextMenuTrigger>
        <ContextMenuContent>
          {node.isFolder && (
            <>
              <ContextMenuItem onClick={() => onNewFile(node.path)}>
                New File
              </ContextMenuItem>
              <ContextMenuItem onClick={() => onNewFolder(node.path)}>
                New Folder
              </ContextMenuItem>
            </>
          )}
          <ContextMenuItem onClick={() => onRename(node)}>
            Rename
          </ContextMenuItem>
          <ContextMenuItem
            className="text-red-400"
            onClick={() => onDelete(node)}
          >
            Delete
          </ContextMenuItem>
        </ContextMenuContent>
      </ContextMenu>

      {node.isFolder &&
        expanded &&
        node.children.map((child) => (
          <TreeItem
            key={child.path}
            node={child}
            depth={depth + 1}
            activeFileId={activeFileId}
            onSelectFile={onSelectFile}
            onNewFile={onNewFile}
            onNewFolder={onNewFolder}
            onDelete={onDelete}
            onRename={onRename}
          />
        ))}
    </>
  );
}

type DialogState =
  | { type: "closed" }
  | { type: "new-file"; folderPath: string }
  | { type: "new-folder"; folderPath: string }
  | { type: "rename"; node: TreeNode };

export function FileTree() {
  const { activeWorkspaceId, activeFileId, setActiveFileId } = useWorkspace();
  const [dialog, setDialog] = useState<DialogState>({ type: "closed" });
  const [inputValue, setInputValue] = useState("");

  const files = useLiveQuery(
    () =>
      activeWorkspaceId
        ? db.files.where("workspaceId").equals(activeWorkspaceId).toArray()
        : [],
    [activeWorkspaceId],
  );

  const tree = useMemo(() => buildTree(files ?? []), [files]);

  async function handleNewFile(folderPath: string) {
    setInputValue("");
    setDialog({ type: "new-file", folderPath });
  }

  async function handleNewFolder(folderPath: string) {
    setInputValue("");
    setDialog({ type: "new-folder", folderPath });
  }

  async function handleNewFileAtRoot() {
    setInputValue("");
    setDialog({ type: "new-file", folderPath: "" });
  }

  async function handleRename(node: TreeNode) {
    setInputValue(node.name);
    setDialog({ type: "rename", node });
  }

  async function handleDelete(node: TreeNode) {
    if (!activeWorkspaceId) return;
    try {
      if (node.isFolder) {
        await deleteFolder(activeWorkspaceId, node.path);
        if (
          activeFileId &&
          files?.find((f) => f.id === activeFileId)?.path.startsWith(
            node.path + "/",
          )
        ) {
          setActiveFileId(null);
        }
      } else if (node.fileId !== undefined) {
        await deleteFile(node.fileId);
        if (activeFileId === node.fileId) {
          setActiveFileId(null);
        }
      }
    } catch (err) {
      toast.error(`Failed to delete: ${err}`);
    }
  }

  async function handleDialogSubmit() {
    if (!activeWorkspaceId || !inputValue.trim()) return;

    try {
      if (dialog.type === "new-file") {
        const path = dialog.folderPath
          ? `${dialog.folderPath}/${inputValue.trim()}`
          : inputValue.trim();
        const id = await createFile(activeWorkspaceId, path);
        setActiveFileId(id);
      } else if (dialog.type === "new-folder") {
        // Create a placeholder file inside the new folder
        const folderName = inputValue.trim();
        const basePath = dialog.folderPath
          ? `${dialog.folderPath}/${folderName}`
          : folderName;
        await createFile(activeWorkspaceId, `${basePath}/.gitkeep`);
      } else if (dialog.type === "rename") {
        if (dialog.node.isFolder) {
          const parts = dialog.node.path.split("/");
          parts[parts.length - 1] = inputValue.trim();
          await renameFolder(activeWorkspaceId, dialog.node.path, parts.join("/"));
        } else if (dialog.node.fileId !== undefined) {
          const parts = dialog.node.path.split("/");
          parts[parts.length - 1] = inputValue.trim();
          await renameFile(dialog.node.fileId, parts.join("/"));
        }
      }
    } catch (err) {
      toast.error(`Operation failed: ${err}`);
    }

    setDialog({ type: "closed" });
  }

  if (!activeWorkspaceId) {
    return (
      <div className="flex h-full items-center justify-center p-4 text-xs text-zinc-600">
        Select a workspace
      </div>
    );
  }

  return (
    <>
      <div className="flex h-full flex-col">
        <div className="flex items-center justify-between border-b border-zinc-800 px-3 py-2">
          <span className="text-xs font-medium text-zinc-500">FILES</span>
          <Button
            variant="ghost"
            size="sm"
            className="h-6 w-6 p-0"
            onClick={handleNewFileAtRoot}
          >
            <File className="h-3.5 w-3.5" />
          </Button>
        </div>
        <ScrollArea className="flex-1">
          <div className="py-1">
            {tree.map((node) => (
              <TreeItem
                key={node.path}
                node={node}
                depth={0}
                activeFileId={activeFileId}
                onSelectFile={setActiveFileId}
                onNewFile={handleNewFile}
                onNewFolder={handleNewFolder}
                onDelete={handleDelete}
                onRename={handleRename}
              />
            ))}
          </div>
        </ScrollArea>
      </div>

      <Dialog
        open={dialog.type !== "closed"}
        onOpenChange={(open) => {
          if (!open) setDialog({ type: "closed" });
        }}
      >
        <DialogContent className="sm:max-w-sm">
          <DialogHeader>
            <DialogTitle>
              {dialog.type === "new-file"
                ? "New File"
                : dialog.type === "new-folder"
                  ? "New Folder"
                  : "Rename"}
            </DialogTitle>
          </DialogHeader>
          <Input
            value={inputValue}
            onChange={(e) => setInputValue(e.target.value)}
            placeholder={dialog.type === "new-folder" ? "folder-name" : "filename.md"}
            onKeyDown={(e) => {
              if (e.key === "Enter") handleDialogSubmit();
            }}
            autoFocus
          />
          <DialogFooter>
            <Button size="sm" onClick={handleDialogSubmit}>
              {dialog.type === "rename" ? "Rename" : "Create"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}
```

- [ ] **Step 2: Verify build**

```bash
cd playground && pnpm build
```

Expected: Build succeeds.

- [ ] **Step 3: Commit**

```bash
git add playground/src/components/FileTree.tsx
git commit -m "feat(playground): FileTree component with CRUD, folders, and context menu"
```

---

### Task 7: Editor component

**Files:**
- Create: `playground/src/components/Editor.tsx`

- [ ] **Step 1: Install Monaco**

```bash
cd playground && pnpm add @monaco-editor/react
```

- [ ] **Step 2: Create `playground/src/components/Editor.tsx`**

```tsx
import { useCallback, useMemo, useRef } from "react";
import MonacoEditor, { type OnMount } from "@monaco-editor/react";
import { useLiveQuery } from "dexie-react-hooks";
import { useWorkspace } from "@/contexts/workspace-context";
import { db, updateFileContent } from "@/vfs/db";
import { getMonacoLanguage } from "@/lib/formats";
import { debounce } from "@/lib/debounce";

export function Editor() {
  const { activeFileId } = useWorkspace();
  const editorRef = useRef<Parameters<OnMount>[0] | null>(null);

  const file = useLiveQuery(
    () => (activeFileId ? db.files.get(activeFileId) : undefined),
    [activeFileId],
  );

  const language = useMemo(
    () => (file ? getMonacoLanguage(file.path) : "plaintext"),
    [file],
  );

  const debouncedSave = useMemo(() => {
    if (!activeFileId) return undefined;
    return debounce((content: string) => {
      updateFileContent(activeFileId, content);
    }, 500);
  }, [activeFileId]);

  const handleMount: OnMount = useCallback((editor) => {
    editorRef.current = editor;
  }, []);

  const handleChange = useCallback(
    (value: string | undefined) => {
      if (value !== undefined && debouncedSave) {
        debouncedSave(value);
      }
    },
    [debouncedSave],
  );

  if (!file) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-zinc-600">
        Select a file to edit
      </div>
    );
  }

  return (
    <MonacoEditor
      height="100%"
      language={language}
      value={file.content}
      theme="vs-dark"
      onMount={handleMount}
      onChange={handleChange}
      options={{
        fontSize: 14,
        fontFamily: "'Geist Mono', monospace",
        minimap: { enabled: false },
        lineNumbers: "on",
        scrollBeyondLastLine: false,
        wordWrap: "on",
        padding: { top: 12 },
        automaticLayout: true,
      }}
    />
  );
}
```

- [ ] **Step 3: Verify build**

```bash
cd playground && pnpm build
```

Expected: Build succeeds.

- [ ] **Step 4: Commit**

```bash
git add playground/src/components/Editor.tsx
git commit -m "feat(playground): Monaco editor with language detection and auto-save"
```

---

### Task 8: OutputTabs component

**Files:**
- Create: `playground/src/components/OutputTabs.tsx`

- [ ] **Step 1: Create `playground/src/components/OutputTabs.tsx`**

```tsx
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";

function Placeholder({ label }: { label: string }) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-2 text-zinc-600">
      <span className="text-sm font-medium">{label}</span>
      <span className="text-xs">Connect docmux WASM to enable conversion</span>
    </div>
  );
}

export function OutputTabs() {
  return (
    <Tabs defaultValue="preview" className="flex h-full flex-col">
      <TabsList className="mx-2 mt-2 w-fit">
        <TabsTrigger value="preview" className="text-xs">
          Preview
        </TabsTrigger>
        <TabsTrigger value="source" className="text-xs">
          Source
        </TabsTrigger>
        <TabsTrigger value="ast" className="text-xs">
          AST
        </TabsTrigger>
        <TabsTrigger value="diagnostics" className="text-xs">
          Diagnostics
        </TabsTrigger>
      </TabsList>

      <TabsContent value="preview" className="flex-1">
        <Placeholder label="Preview" />
      </TabsContent>
      <TabsContent value="source" className="flex-1">
        <Placeholder label="Source Output" />
      </TabsContent>
      <TabsContent value="ast" className="flex-1">
        <Placeholder label="AST Inspector" />
      </TabsContent>
      <TabsContent value="diagnostics" className="flex-1">
        <Placeholder label="Diagnostics" />
      </TabsContent>
    </Tabs>
  );
}
```

- [ ] **Step 2: Verify build**

```bash
cd playground && pnpm build
```

Expected: Build succeeds.

- [ ] **Step 3: Commit**

```bash
git add playground/src/components/OutputTabs.tsx
git commit -m "feat(playground): OutputTabs with placeholder content"
```

---

### Task 9: App shell — wire everything together

**Files:**
- Modify: `playground/src/App.tsx`
- Modify: `playground/src/main.tsx`

Note: `sonner` was already installed by `pnpm dlx shadcn@latest add sonner` in Task 2.

- [ ] **Step 1: Update `playground/src/App.tsx`**

```tsx
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable";
import { Header } from "@/components/Header";
import { FileTree } from "@/components/FileTree";
import { Editor } from "@/components/Editor";
import { OutputTabs } from "@/components/OutputTabs";

export default function App() {
  return (
    <div className="flex h-screen flex-col bg-zinc-950 text-zinc-100">
      <Header />
      <ResizablePanelGroup direction="horizontal" className="flex-1">
        <ResizablePanel defaultSize={20} minSize={15} maxSize={35}>
          <FileTree />
        </ResizablePanel>
        <ResizableHandle withHandle />
        <ResizablePanel defaultSize={40} minSize={25}>
          <Editor />
        </ResizablePanel>
        <ResizableHandle withHandle />
        <ResizablePanel defaultSize={40} minSize={20}>
          <OutputTabs />
        </ResizablePanel>
      </ResizablePanelGroup>
    </div>
  );
}
```

- [ ] **Step 2: Update `playground/src/main.tsx`**

```tsx
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { Toaster } from "sonner";
import { WorkspaceProvider } from "@/contexts/workspace-context";
import { ensureDefaultWorkspace } from "@/vfs/db";
import "./index.css";
import App from "./App";

ensureDefaultWorkspace();

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <WorkspaceProvider>
      <App />
      <Toaster theme="dark" position="bottom-right" />
    </WorkspaceProvider>
  </StrictMode>,
);
```

- [ ] **Step 3: Update `playground/index.html` title**

Ensure the `<title>` is `docmux playground`.

- [ ] **Step 4: Verify dev server**

```bash
cd playground && pnpm dev
```

Expected: App loads with header, file tree (showing "Select a workspace"), editor ("Select a file to edit"), and output tabs with placeholders.

- [ ] **Step 5: Commit**

```bash
git add playground/src/App.tsx playground/src/main.tsx playground/index.html
git commit -m "feat(playground): wire App shell with resizable 3-pane layout"
```

---

### Task 10: Auto-select default workspace on startup

**Files:**
- Modify: `playground/src/main.tsx`
- Modify: `playground/src/contexts/workspace-context.tsx`

- [ ] **Step 1: Add auto-select logic to `WorkspaceProvider`**

Add an effect in `workspace-context.tsx` that auto-selects the first workspace when `activeWorkspaceId` is null and workspaces exist:

```tsx
import { createContext, useContext, useState, useEffect, type ReactNode } from "react";
import { useLiveQuery } from "dexie-react-hooks";
import { db } from "@/vfs/db";

// ... interface and context stay the same ...

export function WorkspaceProvider({ children }: { children: ReactNode }) {
  const [activeWorkspaceId, setActiveWorkspaceId] = useState<number | null>(null);
  const [activeFileId, setActiveFileId] = useState<number | null>(null);

  const workspaces = useLiveQuery(() => db.workspaces.toArray());

  useEffect(() => {
    const firstWs = workspaces?.[0];
    if (activeWorkspaceId === null && firstWs?.id != null) {
      setActiveWorkspaceId(firstWs.id);
    }
  }, [activeWorkspaceId, workspaces]);

  return (
    <WorkspaceContext.Provider
      value={{
        activeWorkspaceId,
        activeFileId,
        setActiveWorkspaceId,
        setActiveFileId,
      }}
    >
      {children}
    </WorkspaceContext.Provider>
  );
}
```

- [ ] **Step 2: Verify full flow**

```bash
cd playground && pnpm dev
```

Expected: App opens → default "Example" workspace is auto-selected → file tree shows `example.md` → click it → Monaco opens with the example content → output tabs show placeholders.

- [ ] **Step 3: Commit**

```bash
git add playground/src/contexts/workspace-context.tsx
git commit -m "feat(playground): auto-select default workspace on startup"
```

---

### Task 11: Final build verification and cleanup

- [ ] **Step 1: Run full build**

```bash
cd playground && pnpm build
```

Expected: Build succeeds with no errors.

- [ ] **Step 2: Run type check**

```bash
cd playground && pnpm exec tsc --noEmit
```

Expected: No type errors.

- [ ] **Step 3: Test the full app flow manually**

```bash
cd playground && pnpm dev
```

Verify:
1. App loads with dark theme
2. "Example" workspace is auto-selected in dropdown
3. File tree shows `example.md`
4. Clicking `example.md` opens it in Monaco with syntax highlighting
5. Editing text and waiting 500ms persists (refresh page — content preserved)
6. "New" button creates a new workspace
7. Right-click file tree → New File, Rename, Delete all work
8. Output tabs show placeholders
9. Resizable panels work via drag handles

- [ ] **Step 4: Commit any final fixes (skip if no changes needed)**

```bash
git add playground/
git commit -m "fix(playground): address build verification issues"
```
