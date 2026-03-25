import { useLiveQuery } from "dexie-react-hooks";
import { FolderOpen, Plus } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useWorkspace } from "@/contexts/workspace-context";
import { db, createWorkspace } from "@/vfs/db";
import { importFolder } from "@/vfs/import";
import { toast } from "sonner";

export function Header() {
  const { activeWorkspaceId, setActiveWorkspaceId, setActiveFileId } =
    useWorkspace();
  const workspaces = useLiveQuery(() => db.workspaces.toArray());

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
    <header className="flex h-12 items-center gap-3 border-b border-zinc-800 bg-zinc-950 px-4">
      <span className="text-sm font-semibold tracking-tight text-zinc-100">
        docmux playground
      </span>

      <HeaderSeparator />

      <Select
        value={activeWorkspaceId?.toString() ?? ""}
        onValueChange={(val) => {
          setActiveWorkspaceId(Number(val));
          setActiveFileId(null);
        }}
      >
        <SelectTrigger className="h-8 w-48 text-xs">
          <SelectValue placeholder="Select workspace" />
        </SelectTrigger>
        <SelectContent>
          {workspaces?.map((ws) => (
            <SelectItem key={ws.id} value={ws.id!.toString()}>
              {ws.name}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>

      <Button
        variant="ghost"
        size="sm"
        className="h-8 gap-1.5 text-xs"
        onClick={handleImportFolder}
      >
        <FolderOpen className="h-3.5 w-3.5" />
        Import Folder
      </Button>

      <Button
        variant="ghost"
        size="sm"
        className="h-8 gap-1.5 text-xs"
        onClick={handleNewWorkspace}
      >
        <Plus className="h-3.5 w-3.5" />
        New
      </Button>
    </header>
  );
}

function HeaderSeparator() {
  return <div className="h-4 w-px bg-zinc-800" />;
}
