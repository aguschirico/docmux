import { useLiveQuery } from "dexie-react-hooks";
import { ChevronDown, FolderOpen, Lock, Package, Plus, Zap } from "lucide-react";
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

function GitHubMark() {
  return (
    <svg viewBox="0 0 16 16" aria-hidden="true" className="h-4 w-4 fill-current">
      <path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z" />
    </svg>
  );
}

function Logo() {
  return (
    <div className="flex items-center gap-2.5">
      <div className="grid h-8 w-8 place-items-center rounded-lg bg-gradient-to-br from-sky-400 to-indigo-500 font-mono text-sm font-bold text-zinc-950 shadow-[0_0_24px_-6px] shadow-sky-500/60">
        d
      </div>
      <span className="font-mono text-lg font-semibold tracking-tight text-zinc-50">docmux</span>
    </div>
  );
}

function Fact({ icon, children }: { icon: React.ReactNode; children: string }) {
  return (
    <span className="inline-flex items-center gap-1.5">
      <span className="text-zinc-500">{icon}</span>
      {children}
    </span>
  );
}

export function Header() {
  return (
    <header className="flex shrink-0 flex-wrap items-center gap-x-8 gap-y-3 border-b border-zinc-800/80 bg-gradient-to-b from-zinc-900/60 to-zinc-950 px-5 py-4">
      <Logo />
      <div className="min-w-0 flex-1">
        <h1 className="text-[15px] font-medium text-zinc-100">
          Convert LaTeX, Markdown, Typst, Word and HTML, right in your browser.
        </h1>
        <p className="mt-1 flex flex-wrap gap-x-4 gap-y-1 text-xs text-zinc-400">
          <Fact icon={<Zap className="h-3 w-3" />}>Rust compiled to WebAssembly</Fact>
          <Fact icon={<Lock className="h-3 w-3" />}>Your files never leave this page</Fact>
          <Fact icon={<Package className="h-3 w-3" />}>MIT licensed</Fact>
        </p>
      </div>
      <nav className="flex items-center gap-2">
        <WorkspaceMenu />
        <a
          href={NPM_URL}
          target="_blank"
          rel="noreferrer"
          className="inline-flex h-8 items-center rounded-md px-3 text-xs text-zinc-300 transition-colors hover:bg-zinc-800 hover:text-zinc-50"
        >
          npm
        </a>
        <a
          href={REPO_URL}
          target="_blank"
          rel="noreferrer"
          className="inline-flex h-8 items-center gap-2 rounded-md border border-zinc-700 bg-zinc-900 px-3 text-xs font-medium text-zinc-100 transition-colors hover:border-zinc-500 hover:bg-zinc-800"
        >
          <GitHubMark />
          GitHub
        </a>
      </nav>
    </header>
  );
}
