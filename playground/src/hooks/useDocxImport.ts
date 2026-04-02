import { useCallback } from "react";
import { useWorkspace } from "@/contexts/workspace-context";
import { db, createBinaryFile } from "@/vfs/db";
import { toast } from "sonner";

export function useDocxImport() {
  const { activeWorkspaceId, setActiveFileId } = useWorkspace();

  const importDocxFile = useCallback(
    async (file: File) => {
      if (!activeWorkspaceId) {
        toast.error("Create or select a workspace first");
        return;
      }

      if (!file.name.toLowerCase().endsWith(".docx")) {
        toast.error("Only .docx files are supported");
        return;
      }

      const buffer = await file.arrayBuffer();

      // Replace existing entry with same name, or create new
      const existing = await db.files
        .where("[workspaceId+path]")
        .equals([activeWorkspaceId, file.name])
        .first();

      let fileId: number;
      if (existing?.id) {
        await db.files.update(existing.id, {
          binaryContent: buffer,
          content: "",
          updatedAt: new Date(),
        });
        fileId = existing.id;
      } else {
        fileId = await createBinaryFile(activeWorkspaceId, file.name, buffer);
      }

      setActiveFileId(fileId);
      toast.success(`Opened ${file.name}`);
    },
    [activeWorkspaceId, setActiveFileId],
  );

  const openFilePicker = useCallback(() => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".docx";
    input.onchange = () => {
      const file = input.files?.[0];
      if (file) importDocxFile(file);
    };
    input.click();
  }, [importDocxFile]);

  return { importDocxFile, openFilePicker };
}
