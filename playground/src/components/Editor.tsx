import MonacoEditor from "@monaco-editor/react";
import { useWorkspace } from "@/contexts/workspace-context";
import { useActiveFile } from "@/components/editor/useActiveFile";
import { getMonacoLanguage } from "@/lib/formats";

function EmptyState({ message }: { message: string }) {
  return (
    <div className="flex h-full items-center justify-center text-sm text-zinc-600">
      {message}
    </div>
  );
}

export function Editor() {
  const { activeWorkspaceId, activeFileId } = useWorkspace();
  const { content, filePath, onChange } = useActiveFile(activeFileId);

  if (!activeWorkspaceId) {
    return <EmptyState message="Select a workspace to start editing" />;
  }

  if (!activeFileId || !filePath) {
    return <EmptyState message="Select a file from the tree" />;
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
