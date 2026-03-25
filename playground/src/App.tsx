import { useEffect } from "react";
import { Toaster } from "sonner";
import { WorkspaceProvider, useWorkspace } from "@/contexts/workspace-context";
import { ensureDefaultWorkspace, db } from "@/vfs/db";
import { Header } from "@/components/Header";
import { FileTree } from "@/components/FileTree";
import { Editor } from "@/components/Editor";
import { OutputTabs } from "@/components/OutputTabs";

function AutoSelectWorkspace() {
  const { activeWorkspaceId, setActiveWorkspaceId } = useWorkspace();

  useEffect(() => {
    if (activeWorkspaceId) return;
    ensureDefaultWorkspace().then(async () => {
      const first = await db.workspaces.toCollection().first();
      if (first?.id) setActiveWorkspaceId(first.id);
    });
  }, [activeWorkspaceId, setActiveWorkspaceId]);

  return null;
}

function Layout() {
  return (
    <div className="flex h-screen flex-col bg-zinc-950 text-zinc-100">
      <Header />
      <div className="flex flex-1 overflow-hidden">
        <div className="w-56 shrink-0 border-r border-zinc-800">
          <FileTree />
        </div>
        <div className="flex-1 min-w-0">
          <Editor />
        </div>
        <div className="w-[420px] shrink-0 border-l border-zinc-800">
          <OutputTabs />
        </div>
      </div>
    </div>
  );
}

export default function App() {
  return (
    <WorkspaceProvider>
      <AutoSelectWorkspace />
      <Layout />
      <Toaster theme="dark" position="bottom-right" />
    </WorkspaceProvider>
  );
}
