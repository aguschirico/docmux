import { useEffect, useState } from "react";
import { Toaster } from "sonner";
import { WorkspaceProvider, useWorkspace } from "@/contexts/workspace-context";
import { db } from "@/vfs/db";
import { loadExample } from "@/vfs/examples";
import { DEFAULT_EXAMPLE, type Example } from "@/examples";
import { Header } from "@/components/Header";
import { ExampleBar } from "@/components/ExampleBar";
import { InputPane } from "@/components/InputPane";
import { OutputPanel } from "@/components/OutputPanel";
import { ResizablePanelGroup, ResizablePanel, ResizableHandle } from "@/components/ui/resizable";
import { useMediaQuery } from "@/hooks/useMediaQuery";

type Pane = "input" | "output";

/** Phones show one pane at a time; the output comes first. */
function PaneSwitch({ pane, onChange }: { pane: Pane; onChange: (pane: Pane) => void }) {
  return (
    <div className="flex shrink-0 gap-1 border-b border-zinc-800/80 p-2">
      {(["input", "output"] as const).map((p) => (
        <button
          key={p}
          onClick={() => onChange(p)}
          aria-pressed={pane === p}
          className={`h-8 flex-1 rounded-md text-sm font-medium capitalize transition-colors ${
            pane === p ? "bg-zinc-800 text-zinc-50" : "text-zinc-400 hover:text-zinc-200"
          }`}
        >
          {p}
        </button>
      ))}
    </div>
  );
}

/**
 * Open something on first render: the default example for a first visit,
 * otherwise the first workspace and file the visitor already has.
 */
function useInitialDocument(onExample: (example: Example) => void) {
  const { activeWorkspaceId, activeFileId, setActiveWorkspaceId, setActiveFileId } = useWorkspace();

  useEffect(() => {
    if (activeWorkspaceId) return;
    db.workspaces.toCollection().first().then(async (first) => {
      if (first?.id) {
        setActiveWorkspaceId(first.id);
        return;
      }
      const { workspaceId, fileId } = await loadExample(DEFAULT_EXAMPLE);
      setActiveWorkspaceId(workspaceId);
      setActiveFileId(fileId);
      onExample(DEFAULT_EXAMPLE);
    });
  }, [activeWorkspaceId, setActiveWorkspaceId, setActiveFileId, onExample]);

  useEffect(() => {
    if (!activeWorkspaceId || activeFileId) return;
    db.files
      .where("workspaceId")
      .equals(activeWorkspaceId)
      .first()
      .then((file) => {
        if (file?.id) setActiveFileId(file.id);
      });
  }, [activeWorkspaceId, activeFileId, setActiveFileId]);
}

function Layout() {
  const [tab, setTab] = useState(DEFAULT_EXAMPLE.output);
  const [onExample] = useState(() => (loaded: Example) => setTab(loaded.output));
  useInitialDocument(onExample);
  const narrow = useMediaQuery("(max-width: 767px)");
  const [pane, setPane] = useState<Pane>("output");

  if (narrow) {
    return (
      <div className="flex h-dvh flex-col bg-zinc-950 text-zinc-100">
        <Header />
        <ExampleBar onLoaded={onExample} />
        <PaneSwitch pane={pane} onChange={setPane} />
        <div className="min-h-0 flex-1">
          {pane === "input" ? <InputPane /> : <OutputPanel tabId={tab} onTabChange={setTab} />}
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-dvh flex-col bg-zinc-950 text-zinc-100">
      <Header />
      <ExampleBar onLoaded={onExample} />
      <ResizablePanelGroup orientation="horizontal" className="min-h-0 flex-1">
        <ResizablePanel defaultSize={50} minSize={25}>
          <InputPane />
        </ResizablePanel>
        <ResizableHandle className="bg-zinc-800 transition-colors hover:bg-sky-500/60 active:bg-sky-400" />
        <ResizablePanel defaultSize={50} minSize={25}>
          <OutputPanel tabId={tab} onTabChange={setTab} />
        </ResizablePanel>
      </ResizablePanelGroup>
    </div>
  );
}

export default function App() {
  return (
    <WorkspaceProvider>
      <Layout />
      <Toaster theme="dark" position="bottom-right" />
    </WorkspaceProvider>
  );
}
