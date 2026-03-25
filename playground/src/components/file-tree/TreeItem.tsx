import { useState } from "react";
import {
  File,
  Folder,
  FolderOpen,
  FileText,
  FileCode,
  ChevronRight,
  ChevronDown,
} from "lucide-react";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import type { TreeNode } from "@/lib/file-tree";
import { getExtension } from "@/lib/formats";

function getFileIcon(name: string) {
  const ext = getExtension(name);
  switch (ext) {
    case "md":
    case "markdown":
      return <FileText className="h-4 w-4 shrink-0 text-zinc-500" />;
    case "tex":
    case "latex":
    case "typ":
      return <FileCode className="h-4 w-4 shrink-0 text-zinc-500" />;
    default:
      return <File className="h-4 w-4 shrink-0 text-zinc-500" />;
  }
}

export interface TreeItemProps {
  node: TreeNode;
  depth: number;
  activeFileId: number | null;
  onSelectFile: (id: number) => void;
  onNewFile: (folderPath: string) => void;
  onNewFolder: (folderPath: string) => void;
  onDelete: (node: TreeNode) => void;
  onRename: (node: TreeNode) => void;
}

export function TreeItem({
  node,
  depth,
  activeFileId,
  onSelectFile,
  onNewFile,
  onNewFolder,
  onDelete,
  onRename,
}: TreeItemProps) {
  const [expanded, setExpanded] = useState(depth === 0);
  const isActive = !node.isFolder && node.fileId === activeFileId;

  return (
    <>
      <ContextMenu>
        <ContextMenuTrigger
          render={<button type="button" />}
          className={`flex w-full items-center gap-1.5 rounded-sm px-2 py-1 text-left text-xs hover:bg-zinc-800 ${
            isActive ? "bg-zinc-800 text-zinc-100" : "text-zinc-400"
          }`}
          style={{ paddingLeft: `${depth * 12 + 8}px` }}
          onClick={() => {
            if (node.isFolder) {
              setExpanded(!expanded);
            } else if (node.fileId !== undefined) {
              onSelectFile(node.fileId);
            }
          }}
        >
          {node.isFolder ? (
            <>
              {expanded ? (
                <ChevronDown className="h-3.5 w-3.5 shrink-0 text-zinc-600" />
              ) : (
                <ChevronRight className="h-3.5 w-3.5 shrink-0 text-zinc-600" />
              )}
              {expanded ? (
                <FolderOpen className="h-4 w-4 shrink-0 text-zinc-500" />
              ) : (
                <Folder className="h-4 w-4 shrink-0 text-zinc-500" />
              )}
            </>
          ) : (
            <>
              <span className="w-3.5" />
              {getFileIcon(node.name)}
            </>
          )}
          <span className="truncate">{node.name}</span>
        </ContextMenuTrigger>
        <ContextMenuContent>
          {node.isFolder && (
            <>
              <ContextMenuItem onClick={() => onNewFile(node.path)}>
                New File
              </ContextMenuItem>
              <ContextMenuItem onClick={() => onNewFolder(node.path)}>
                New Folder
              </ContextMenuItem>
            </>
          )}
          <ContextMenuItem onClick={() => onRename(node)}>
            Rename
          </ContextMenuItem>
          <ContextMenuItem
            className="text-red-400"
            onClick={() => onDelete(node)}
          >
            Delete
          </ContextMenuItem>
        </ContextMenuContent>
      </ContextMenu>

      {node.isFolder &&
        expanded &&
        node.children.map((child) => (
          <TreeItem
            key={child.path}
            node={child}
            depth={depth + 1}
            activeFileId={activeFileId}
            onSelectFile={onSelectFile}
            onNewFile={onNewFile}
            onNewFolder={onNewFolder}
            onDelete={onDelete}
            onRename={onRename}
          />
        ))}
    </>
  );
}
