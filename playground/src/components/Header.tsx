import { useLiveQuery } from "dexie-react-hooks";
import { ChevronDown, ExternalLink, FolderOpen, Plus } from "lucide-react";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useWorkspace } from "@/contexts/workspace-context";
import { db, createWorkspace } from "@/vfs/db";
import { importFolder } from "@/vfs/import";
import { toast } from "sonner";

const REPO_URL = "https://github.com/aguschirico/docmux";
const NPM_URL = "https://www.npmjs.com/package/@docmux/wasm";

function WorkspaceMenu() {
  const { activeWorkspaceId, setActiveWorkspaceId, setActiveFileId } = useWorkspace();
  const workspaces = useLiveQuery(() => db.workspaces.toArray());
  const active = workspaces?.find((ws) => ws.id === activeWorkspaceId);

  async function select(id: number | null) {
    if (id === null) return;
    setActiveWorkspaceId(id);
    setActiveFileId(null);
  }

  async function handleNew() {
    try {
      await select(await createWorkspace(`Workspace ${(workspaces?.length ?? 0) + 1}`));
    } catch (err) {
      toast.error(`Could not create workspace: ${err}`);
    }
  }

  async function handleImport() {
    try {
      await select(await importFolder());
    } catch (err) {
      toast.error(`Could not import folder: ${err}`);
    }
  }

  return (
    <DropdownMenu>
      <DropdownMenuTrigger className="inline-flex h-8 items-center gap-1.5 rounded-md px-2.5 text-xs text-zinc-400 transition-colors hover:bg-zinc-800 hover:text-zinc-100">
        <span className="max-w-36 truncate">{active?.name ?? "Workspace"}</span>
        <ChevronDown className="h-3 w-3" />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        {workspaces?.map((ws) => (
          <DropdownMenuItem key={ws.id} onClick={() => select(ws.id ?? null)}>
            {ws.name}
          </DropdownMenuItem>
        ))}
        <DropdownMenuSeparator />
        <DropdownMenuItem onClick={handleImport}>
          <FolderOpen className="h-3.5 w-3.5" /> Import folder
        </DropdownMenuItem>
        <DropdownMenuItem onClick={handleNew}>
          <Plus className="h-3.5 w-3.5" /> New workspace
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function OutLink({ href, children }: { href: string; children: string }) {
  return (
    <a
      href={href}
      target="_blank"
      rel="noreferrer"
      className="inline-flex h-8 items-center gap-1 rounded-md px-2.5 text-xs text-zinc-300 transition-colors hover:bg-zinc-800 hover:text-zinc-100"
    >
      {children}
      <ExternalLink className="h-3 w-3 text-zinc-500" />
    </a>
  );
}

export function Header() {
  return (
    <header className="flex shrink-0 flex-wrap items-center gap-x-6 gap-y-2 border-b border-zinc-800 px-5 py-3">
      <div className="flex min-w-0 flex-1 items-baseline gap-4">
        <span className="text-lg font-semibold tracking-tight text-zinc-50">docmux</span>
        <div className="min-w-0">
          <p className="truncate text-sm text-zinc-200">
            Convert LaTeX, Markdown, Typst, Word and HTML, right in your browser.
          </p>
          <p className="truncate text-xs text-zinc-500">
            Rust compiled to WebAssembly · your files never leave this page · MIT licensed
          </p>
        </div>
      </div>
      <nav className="flex items-center gap-1">
        <WorkspaceMenu />
        <OutLink href={REPO_URL}>GitHub</OutLink>
        <OutLink href={NPM_URL}>npm</OutLink>
      </nav>
    </header>
  );
}
