import { FileUp } from "lucide-react";
import { toast } from "sonner";
import { EXAMPLES, type Example } from "@/examples";
import { useWorkspace } from "@/contexts/workspace-context";
import { useDocxImport } from "@/hooks/useDocxImport";
import { loadExample } from "@/vfs/examples";

interface ExampleBarProps {
  activeExample: string | null;
  onLoaded: (example: Example) => void;
}

export function ExampleBar({ activeExample, onLoaded }: ExampleBarProps) {
  const { setActiveWorkspaceId, setActiveFileId } = useWorkspace();
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
    <div className="flex shrink-0 flex-wrap items-center gap-2 border-b border-zinc-800 px-5 py-2.5">
      <span className="mr-1 text-xs text-zinc-500">Try</span>
      {EXAMPLES.map((example) => {
        const active = example.id === activeExample;
        return (
          <button
            key={example.id}
            onClick={() => open(example)}
            className={`inline-flex h-8 items-center gap-2 rounded-full border px-3.5 text-xs transition-colors ${
              active
                ? "border-sky-500/60 bg-sky-500/10 text-sky-200"
                : "border-zinc-800 text-zinc-300 hover:border-zinc-600 hover:text-zinc-100"
            }`}
          >
            {example.label}
            <span className={active ? "text-sky-400/80" : "text-zinc-500"}>{example.format}</span>
          </button>
        );
      })}
      <div className="mx-1 h-4 w-px bg-zinc-800" />
      <button
        onClick={openFilePicker}
        className="inline-flex h-8 items-center gap-1.5 rounded-full border border-zinc-800 px-3.5 text-xs text-zinc-300 transition-colors hover:border-zinc-600 hover:text-zinc-100"
      >
        <FileUp className="h-3.5 w-3.5" />
        Open a .docx
      </button>
      <span className="text-xs text-zinc-600">or drop a file on the editor</span>
    </div>
  );
}
