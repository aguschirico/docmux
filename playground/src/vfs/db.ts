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

  const wsId = (await db.workspaces.add({
    name: "Example",
    createdAt: new Date(),
  })) as number;

  await db.files.add({
    workspaceId: wsId,
    path: "example.md",
    content: EXAMPLE_MD,
    updatedAt: new Date(),
  });
}

export async function createWorkspace(name: string): Promise<number> {
  return (await db.workspaces.add({
    name,
    createdAt: new Date(),
  })) as number;
}

export async function createFile(
  workspaceId: number,
  path: string,
  content = "",
): Promise<number> {
  return (await db.files.add({
    workspaceId,
    path,
    content,
    updatedAt: new Date(),
  })) as number;
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
