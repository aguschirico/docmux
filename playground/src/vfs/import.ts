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
