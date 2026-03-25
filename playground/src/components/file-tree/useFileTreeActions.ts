import { useState } from "react";
import { useWorkspace } from "@/contexts/workspace-context";
import {
  createFile,
  renameFile,
  renameFolder,
  deleteFile,
  deleteFolder,
} from "@/vfs/db";
import type { TreeNode } from "@/lib/file-tree";
import type { DialogState } from "./FileTreeDialog";
import type { VfsFile } from "@/vfs/types";
import { toast } from "sonner";

export function useFileTreeActions(files: VfsFile[] | undefined) {
  const { activeWorkspaceId, activeFileId, setActiveFileId } = useWorkspace();
  const [dialog, setDialog] = useState<DialogState>({ type: "closed" });
  const [inputValue, setInputValue] = useState("");

  function openNewFile(folderPath: string) {
    setInputValue("");
    setDialog({ type: "new-file", folderPath });
  }

  function openNewFolder(folderPath: string) {
    setInputValue("");
    setDialog({ type: "new-folder", folderPath });
  }

  function openRename(node: TreeNode) {
    setInputValue(node.name);
    setDialog({ type: "rename", node });
  }

  async function handleDelete(node: TreeNode) {
    if (!activeWorkspaceId) return;
    try {
      if (node.isFolder) {
        await deleteFolder(activeWorkspaceId, node.path);
        const activeInFolder = files?.find(
          (f) => f.id === activeFileId,
        )?.path.startsWith(node.path + "/");
        if (activeFileId && activeInFolder) setActiveFileId(null);
      } else if (node.fileId !== undefined) {
        await deleteFile(node.fileId);
        if (activeFileId === node.fileId) setActiveFileId(null);
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
        const basePath = dialog.folderPath
          ? `${dialog.folderPath}/${inputValue.trim()}`
          : inputValue.trim();
        await createFile(activeWorkspaceId, `${basePath}/.gitkeep`);
      } else if (dialog.type === "rename") {
        const parts = dialog.node.path.split("/");
        parts[parts.length - 1] = inputValue.trim();
        const newPath = parts.join("/");
        if (dialog.node.isFolder) {
          await renameFolder(activeWorkspaceId, dialog.node.path, newPath);
        } else if (dialog.node.fileId !== undefined) {
          await renameFile(dialog.node.fileId, newPath);
        }
      }
    } catch (err) {
      toast.error(`Operation failed: ${err}`);
    }
    setDialog({ type: "closed" });
  }

  return {
    dialog,
    inputValue,
    setInputValue,
    openNewFile,
    openNewFolder,
    openRename,
    handleDelete,
    handleDialogSubmit,
    closeDialog: () => setDialog({ type: "closed" }),
  };
}
