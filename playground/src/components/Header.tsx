import { useLiveQuery } from "dexie-react-hooks";
import { FolderOpen, Plus, ChevronDown, FileUp } from "lucide-react";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useWorkspace } from "@/contexts/workspace-context";
import { db, createWorkspace } from "@/vfs/db";
import { importFolder } from "@/vfs/import";
import { useDocxImport } from "@/hooks/useDocxImport";
import { toast } from "sonner";

export function Header() {
  const { activeWorkspaceId, setActiveWorkspaceId, setActiveFileId } =
    useWorkspace();
  const workspaces = useLiveQuery(() => db.workspaces.toArray());

  const { openFilePicker: openDocx } = useDocxImport();
  const activeWs = workspaces?.find((ws) => ws.id === activeWorkspaceId);

  async function handleNewWorkspace() {
    try {
      const name = `Workspace ${(workspaces?.length ?? 0) + 1}`;
      const id = await createWorkspace(name);
      setActiveWorkspaceId(id);
      setActiveFileId(null);
    } catch (err) {
      toast.error(`Failed to create workspace: ${err}`);
    }
  }

  async function handleImportFolder() {
    try {
      const wsId = await importFolder();
      if (wsId !== null) {
        setActiveWorkspaceId(wsId);
        setActiveFileId(null);
      }
    } catch (err) {
      toast.error(`Failed to import folder: ${err}`);
    }
  }

  return (
    <header className="flex h-11 shrink-0 items-center gap-3 border-b border-zinc-800 bg-zinc-950 px-4">
      <span className="text-sm font-semibold tracking-tight text-zinc-100">
        docmux
      </span>

      <div className="h-4 w-px bg-zinc-800" />

      <DropdownMenu>
        <DropdownMenuTrigger
          className="inline-flex h-7 items-center gap-1.5 rounded-md px-2 text-xs text-zinc-300 hover:bg-zinc-800 hover:text-zinc-100 transition-colors"
        >
          <span className="max-w-40 truncate">
            {activeWs?.name ?? "Select workspace"}
          </span>
          <ChevronDown className="h-3 w-3 text-zinc-500" />
        </DropdownMenuTrigger>
        <DropdownMenuContent align="start">
          {workspaces?.map((ws) => (
            <DropdownMenuItem
              key={ws.id}
              onClick={() => {
                setActiveWorkspaceId(ws.id!);
                setActiveFileId(null);
              }}
            >
              {ws.name}
            </DropdownMenuItem>
          ))}
        </DropdownMenuContent>
      </DropdownMenu>

      <div className="flex-1" />

      <button
        className="inline-flex h-7 items-center gap-1.5 rounded-md px-2 text-xs text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100 transition-colors"
        onClick={openDocx}
      >
        <FileUp className="h-3.5 w-3.5" />
        Open .docx
      </button>

      <button
        className="inline-flex h-7 items-center gap-1.5 rounded-md px-2 text-xs text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100 transition-colors"
        onClick={handleImportFolder}
      >
        <FolderOpen className="h-3.5 w-3.5" />
        Import
      </button>

      <button
        className="inline-flex h-7 items-center gap-1.5 rounded-md px-2 text-xs text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100 transition-colors"
        onClick={handleNewWorkspace}
      >
        <Plus className="h-3.5 w-3.5" />
        New
      </button>
    </header>
  );
}
