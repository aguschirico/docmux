import { useEffect } from "react";
import { Toaster } from "sonner";
import { WorkspaceProvider, useWorkspace } from "@/contexts/workspace-context";
import { ensureDefaultWorkspace, db } from "@/vfs/db";
import { Header } from "@/components/Header";
import { FileTree } from "@/components/FileTree";
import { Editor } from "@/components/Editor";
import { OutputTabs } from "@/components/OutputTabs";
import {
  ResizablePanelGroup,
  ResizablePanel,
  ResizableHandle,
} from "@/components/ui/resizable";

function AutoInit() {
  const {
    activeWorkspaceId,
    activeFileId,
    setActiveWorkspaceId,
    setActiveFileId,
  } = useWorkspace();

  // Ensure a default workspace exists, then select it
  useEffect(() => {
    if (activeWorkspaceId) return;
    ensureDefaultWorkspace().then(async () => {
      const first = await db.workspaces.toCollection().first();
      if (first?.id) setActiveWorkspaceId(first.id);
    });
  }, [activeWorkspaceId, setActiveWorkspaceId]);

  // Auto-select the first file when a workspace is active but no file is selected
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

  return null;
}

function Layout() {
  return (
    <div className="flex h-screen flex-col bg-zinc-950 text-zinc-100">
      <Header />
      <ResizablePanelGroup
        direction="horizontal"
        className="flex-1 overflow-hidden"
      >
        <ResizablePanel defaultSize="200px" minSize="160px" maxSize="350px">
          <FileTree />
        </ResizablePanel>
        <ResizableHandle className="bg-zinc-800 transition-colors hover:bg-zinc-500 active:bg-zinc-400" />
        <ResizablePanel defaultSize={42} minSize={20}>
          <Editor />
        </ResizablePanel>
        <ResizableHandle className="bg-zinc-800 transition-colors hover:bg-zinc-500 active:bg-zinc-400" />
        <ResizablePanel defaultSize={40} minSize={20}>
          <OutputTabs />
        </ResizablePanel>
      </ResizablePanelGroup>
    </div>
  );
}

export default function App() {
  return (
    <WorkspaceProvider>
      <AutoInit />
      <Layout />
      <Toaster theme="dark" position="bottom-right" />
    </WorkspaceProvider>
  );
}
