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
  const [example, setExample] = useState<string | null>(null);
  const [onExample] = useState(() => (loaded: Example) => {
    setExample(loaded.id);
    setTab(loaded.output);
  });
  useInitialDocument(onExample);

  return (
    <div className="flex h-dvh flex-col bg-zinc-950 text-zinc-100">
      <Header />
      <ExampleBar activeExample={example} onLoaded={onExample} />
      <ResizablePanelGroup orientation="horizontal" className="min-h-0 flex-1">
        <ResizablePanel defaultSize={50} minSize={25}>
          <InputPane />
        </ResizablePanel>
        <ResizableHandle className="bg-zinc-800 transition-colors hover:bg-zinc-500 active:bg-zinc-400" />
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
