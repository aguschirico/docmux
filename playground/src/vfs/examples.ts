import type { Example } from "@/examples";
import { db } from "./db";

/** Examples load into their own workspace, so user files are never touched. */
const EXAMPLES_WORKSPACE = "Examples";

/**
 * Replace the Examples workspace's files with `example`'s and return the
 * workspace id and the id of its main file.
 */
export async function loadExample(
  example: Example,
): Promise<{ workspaceId: number; fileId: number }> {
  return db.transaction("rw", [db.workspaces, db.files], async () => {
    const existing = await db.workspaces.where("name").equals(EXAMPLES_WORKSPACE).first();
    const workspaceId =
      existing?.id ??
      ((await db.workspaces.add({ name: EXAMPLES_WORKSPACE, createdAt: new Date() })) as number);
    await db.files.where("workspaceId").equals(workspaceId).delete();

    let fileId = 0;
    for (const file of example.files) {
      const id = (await db.files.add({
        workspaceId,
        path: file.path,
        content: file.content,
        updatedAt: new Date(),
      })) as number;
      if (file.path === example.main) fileId = id;
    }
    return { workspaceId, fileId };
  });
}
