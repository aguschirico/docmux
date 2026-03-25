import { useMemo } from "react";
import { useLiveQuery } from "dexie-react-hooks";
import { FilePlus } from "lucide-react";
import { ScrollArea } from "@/components/ui/scroll-area";
import { useWorkspace } from "@/contexts/workspace-context";
import { db } from "@/vfs/db";
import { buildTree } from "@/lib/file-tree";
import { TreeItem } from "@/components/file-tree/TreeItem";
import { FileTreeDialog } from "@/components/file-tree/FileTreeDialog";
import { useFileTreeActions } from "@/components/file-tree/useFileTreeActions";

export function FileTree() {
  const { activeWorkspaceId, activeFileId, setActiveFileId } = useWorkspace();

  const files = useLiveQuery(
    () =>
      activeWorkspaceId
        ? db.files.where("workspaceId").equals(activeWorkspaceId).toArray()
        : [],
    [activeWorkspaceId],
  );

  const tree = useMemo(() => buildTree(files ?? []), [files]);

  const actions = useFileTreeActions(files);

  if (!activeWorkspaceId) {
    return (
      <div className="flex h-full items-center justify-center p-4 text-xs text-zinc-600">
        Select a workspace
      </div>
    );
  }

  return (
    <>
      <div className="flex h-full min-w-[180px] flex-col">
        <div className="flex items-center justify-between border-b border-zinc-800 px-3 py-1.5">
          <span className="text-xs font-medium uppercase tracking-wider text-zinc-500">
            Files
          </span>
          <button
            className="rounded p-0.5 text-zinc-500 hover:bg-zinc-800 hover:text-zinc-300 transition-colors"
            onClick={() => actions.openNewFile("")}
          >
            <FilePlus className="h-3.5 w-3.5" />
          </button>
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
                onNewFile={actions.openNewFile}
                onNewFolder={actions.openNewFolder}
                onDelete={actions.handleDelete}
                onRename={actions.openRename}
              />
            ))}
          </div>
        </ScrollArea>
      </div>

      <FileTreeDialog
        dialog={actions.dialog}
        inputValue={actions.inputValue}
        onInputChange={actions.setInputValue}
        onSubmit={actions.handleDialogSubmit}
        onClose={actions.closeDialog}
      />
    </>
  );
}
