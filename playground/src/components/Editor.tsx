import { FileText, FolderOpen } from "lucide-react";
import MonacoEditor from "@monaco-editor/react";
import { useWorkspace } from "@/contexts/workspace-context";
import { useActiveFile } from "@/components/editor/useActiveFile";
import { getMonacoLanguage } from "@/lib/formats";

interface EmptyStateProps {
  icon: React.ReactNode;
  title: string;
  description: string;
}

function EmptyState({ icon, title, description }: EmptyStateProps) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-3 px-6 text-center">
      <div className="rounded-lg border border-zinc-800 bg-zinc-900/50 p-3 text-zinc-600">
        {icon}
      </div>
      <div className="space-y-1">
        <p className="text-sm font-medium text-zinc-400">{title}</p>
        <p className="text-xs text-zinc-600">{description}</p>
      </div>
    </div>
  );
}

export function Editor() {
  const { activeWorkspaceId, activeFileId } = useWorkspace();
  const { content, filePath, onChange } = useActiveFile(activeFileId);

  if (!activeWorkspaceId) {
    return (
      <EmptyState
        icon={<FolderOpen className="h-5 w-5" />}
        title="No workspace selected"
        description="Create or import a workspace to start editing"
      />
    );
  }

  if (!activeFileId || !filePath) {
    return (
      <EmptyState
        icon={<FileText className="h-5 w-5" />}
        title="No file selected"
        description="Pick a file from the sidebar to open it here"
      />
    );
  }

  return (
    <MonacoEditor
      height="100%"
      theme="vs-dark"
      language={getMonacoLanguage(filePath)}
      value={content}
      onChange={onChange}
      options={{
        minimap: { enabled: false },
        fontSize: 13,
        fontFamily: "JetBrains Mono, Fira Code, monospace",
        lineNumbers: "on",
        scrollBeyondLastLine: false,
        wordWrap: "on",
        padding: { top: 12 },
        renderLineHighlight: "gutter",
        bracketPairColorization: { enabled: true },
        tabSize: 2,
      }}
    />
  );
}
