import { useCallback } from "react";
import { useWorkspace } from "@/contexts/workspace-context";
import { db, createBinaryFile } from "@/vfs/db";
import { toast } from "sonner";

/**
 * Returns a callback that stores a dropped image in the VFS
 * and returns the filename for insertion in the editor.
 */
export function useImageDrop(): (file: File) => Promise<string | null> {
  const { activeWorkspaceId } = useWorkspace();

  return useCallback(
    async (file: File): Promise<string | null> => {
      if (!activeWorkspaceId) return null;

      const buffer = await file.arrayBuffer();
      let filename = file.name;

      // Dedup: check if filename already exists in workspace
      const existing = await db.files
        .where("[workspaceId+path]")
        .equals([activeWorkspaceId, filename])
        .first();

      if (existing) {
        const dot = filename.lastIndexOf(".");
        const base = dot >= 0 ? filename.slice(0, dot) : filename;
        const ext = dot >= 0 ? filename.slice(dot) : "";
        let n = 1;
        while (
          await db.files
            .where("[workspaceId+path]")
            .equals([activeWorkspaceId, `${base}-${n}${ext}`])
            .first()
        ) {
          n++;
        }
        filename = `${base}-${n}${ext}`;
      }

      await createBinaryFile(activeWorkspaceId, filename, buffer);
      toast.success(`Added ${filename}`);
      return filename;
    },
    [activeWorkspaceId],
  );
}
