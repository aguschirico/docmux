import { useState } from "react";
import { useLiveQuery } from "dexie-react-hooks";
import { PanelLeft } from "lucide-react";
import { useWorkspace } from "@/contexts/workspace-context";
import { db } from "@/vfs/db";
import { Editor } from "@/components/Editor";
import { getFormat } from "@/lib/formats";
import { formatBadge } from "@/lib/format-badges";
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
  const activePath = files?.find((f) => f.id === activeFileId)?.path;
  const activeFormat = activePath ? getFormat(activePath) : null;

  return (
    <div className="flex h-full flex-col">
      <div className="flex h-11 shrink-0 items-center gap-1 border-b border-zinc-800/80 bg-zinc-900/40 px-2">
        <button
          onClick={() => setShowTree((v) => !v)}
          title={showTree ? "Hide files" : "Show all files"}
          className={`inline-flex h-7 w-7 items-center justify-center rounded-md transition-colors hover:bg-zinc-800 ${
            showTree ? "text-zinc-100" : "text-zinc-500"
          }`}
        >
          <PanelLeft className="h-4 w-4" />
        </button>
        <span className="px-1.5 text-[11px] font-semibold uppercase tracking-wider text-zinc-500">
          Input
        </span>
        {activeFormat && (
          <span
            className={`mr-1 inline-flex h-5 items-center rounded px-1.5 font-mono text-[10px] font-semibold ring-1 ring-inset ${formatBadge(activeFormat).className}`}
          >
            {formatBadge(activeFormat).short}
          </span>
        )}
        <div className="flex min-w-0 items-center gap-0.5 overflow-x-auto">
          {tabs.map((file) => (
            <button
              key={file.id}
              onClick={() => setActiveFileId(file.id ?? null)}
              className={`h-7 shrink-0 rounded-md px-2.5 font-mono text-xs transition-colors ${
                file.id === activeFileId
                  ? "bg-zinc-800 text-zinc-50 ring-1 ring-inset ring-zinc-700"
                  : "text-zinc-400 hover:bg-zinc-800/50 hover:text-zinc-200"
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
