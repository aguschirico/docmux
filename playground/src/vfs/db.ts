import Dexie, { type EntityTable } from "dexie";
import type { VfsWorkspace, VfsFile } from "./types";

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

export async function createBinaryFile(
  workspaceId: number,
  path: string,
  binaryContent: ArrayBuffer,
): Promise<number> {
  return (await db.files.add({
    workspaceId,
    path,
    content: "",
    binaryContent,
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
