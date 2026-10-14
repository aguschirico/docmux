import { useState } from "react";
import { useLiveQuery } from "dexie-react-hooks";
import { PanelLeft } from "lucide-react";
import { useWorkspace } from "@/contexts/workspace-context";
import { db } from "@/vfs/db";
import { Editor } from "@/components/Editor";
import { FileTree } from "@/components/FileTree";

/** Most projects have a handful of files: show them as tabs. */
const MAX_TABS = 6;

export function InputPane() {
  const { activeWorkspaceId, activeFileId, setActiveFileId } = useWorkspace();
  const [showTree, setShowTree] = useState(false);
  const files = useLiveQuery(
    () =>
      activeWorkspaceId
        ? db.files.where("workspaceId").equals(activeWorkspaceId).sortBy("path")
        : [],
    [activeWorkspaceId],
  );
  const tabs = (files ?? []).slice(0, MAX_TABS);

  return (
    <div className="flex h-full flex-col">
      <div className="flex h-10 shrink-0 items-center gap-1 border-b border-zinc-800 px-2">
        <button
          onClick={() => setShowTree((v) => !v)}
          title={showTree ? "Hide files" : "Show all files"}
          className={`inline-flex h-7 w-7 items-center justify-center rounded-md transition-colors hover:bg-zinc-800 ${
            showTree ? "text-zinc-100" : "text-zinc-500"
          }`}
        >
          <PanelLeft className="h-4 w-4" />
        </button>
        <span className="px-1.5 text-[11px] font-medium uppercase tracking-wider text-zinc-500">
          Input
        </span>
        <div className="flex min-w-0 items-center gap-0.5 overflow-x-auto">
          {tabs.map((file) => (
            <button
              key={file.id}
              onClick={() => setActiveFileId(file.id ?? null)}
              className={`h-7 shrink-0 rounded-md px-2.5 font-mono text-xs transition-colors ${
                file.id === activeFileId
                  ? "bg-zinc-800 text-zinc-100"
                  : "text-zinc-400 hover:text-zinc-200"
              }`}
            >
              {file.path}
            </button>
          ))}
          {(files?.length ?? 0) > MAX_TABS && (
            <button
              onClick={() => setShowTree(true)}
              className="h-7 shrink-0 px-2 text-xs text-zinc-500 hover:text-zinc-300"
            >
              +{(files?.length ?? 0) - MAX_TABS} more
            </button>
          )}
        </div>
      </div>
      <div className="flex min-h-0 flex-1">
        {showTree && (
          <div className="w-56 shrink-0 border-r border-zinc-800">
            <FileTree />
          </div>
        )}
        <div className="min-w-0 flex-1">
          <Editor />
        </div>
      </div>
    </div>
  );
}
