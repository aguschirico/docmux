import { FileUp } from "lucide-react";
import { toast } from "sonner";
import { EXAMPLES, type Example } from "@/examples";
import { useWorkspace } from "@/contexts/workspace-context";
import { useDocxImport } from "@/hooks/useDocxImport";
import { useLiveQuery } from "dexie-react-hooks";
import { db } from "@/vfs/db";
import { EXAMPLES_WORKSPACE, loadExample } from "@/vfs/examples";
import { getFormat } from "@/lib/formats";
import { formatBadge } from "@/lib/format-badges";

interface ExampleBarProps {
  onLoaded: (example: Example) => void;
}

/** The example open in the Examples workspace, if that workspace is active. */
function useActiveExample(): string | null {
  const { activeWorkspaceId } = useWorkspace();
  return (
    useLiveQuery(async () => {
      if (!activeWorkspaceId) return null;
      const workspace = await db.workspaces.get(activeWorkspaceId);
      if (workspace?.name !== EXAMPLES_WORKSPACE) return null;
      const paths = new Set(
        (await db.files.where("workspaceId").equals(activeWorkspaceId).toArray()).map((f) => f.path),
      );
      return EXAMPLES.find((e) => paths.has(e.main))?.id ?? null;
    }, [activeWorkspaceId]) ?? null
  );
}

export function ExampleBar({ onLoaded }: ExampleBarProps) {
  const { setActiveWorkspaceId, setActiveFileId } = useWorkspace();
  const activeExample = useActiveExample();
  const { openFilePicker } = useDocxImport();

  async function open(example: Example) {
    try {
      const { workspaceId, fileId } = await loadExample(example);
      setActiveWorkspaceId(workspaceId);
      setActiveFileId(fileId);
      onLoaded(example);
    } catch (err) {
      toast.error(`Could not load example: ${err}`);
    }
  }

  return (
    <div className="flex shrink-0 flex-wrap items-center gap-2 border-b border-zinc-800/80 px-5 py-3">
      <span className="mr-1 text-xs font-medium text-zinc-400">Try an example</span>
      {EXAMPLES.map((example) => {
        const active = example.id === activeExample;
        const badge = formatBadge(getFormat(example.main));
        return (
          <button
            key={example.id}
            onClick={() => open(example)}
            aria-pressed={active}
            className={`group inline-flex h-9 items-center gap-2.5 rounded-lg border pl-1.5 pr-3.5 text-sm transition-all ${
              active
                ? "border-sky-500/50 bg-sky-500/10 text-zinc-50 shadow-[0_0_0_3px] shadow-sky-500/10"
                : "border-zinc-800 bg-zinc-900/60 text-zinc-300 hover:border-zinc-600 hover:bg-zinc-900 hover:text-zinc-50"
            }`}
          >
            <span
              className={`inline-flex h-6 min-w-9 items-center justify-center rounded-md px-1.5 font-mono text-[10px] font-semibold ring-1 ring-inset ${badge.className}`}
            >
              {badge.short}
            </span>
            {example.label}
          </button>
        );
      })}
      <div className="mx-2 h-5 w-px bg-zinc-800" />
      <button
        onClick={openFilePicker}
        className="inline-flex h-9 items-center gap-2 rounded-lg border border-dashed border-zinc-700 px-3.5 text-sm text-zinc-300 transition-colors hover:border-zinc-500 hover:text-zinc-50"
      >
        <FileUp className="h-4 w-4 text-zinc-500" />
        Open a .docx
      </button>
      <span className="text-xs text-zinc-500">or drop a file on the editor</span>
    </div>
  );
}
