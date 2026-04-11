import { useRef, useCallback } from "react";
import { FileText, FolderOpen, FileUp } from "lucide-react";
import MonacoEditor from "@monaco-editor/react";
import type { editor as monacoEditor } from "monaco-editor";
import { useWorkspace } from "@/contexts/workspace-context";
import { useActiveFile } from "@/components/editor/useActiveFile";
import { useDocxImport } from "@/hooks/useDocxImport";
import { useDropZone } from "@/hooks/useDropZone";
import { useImageDrop } from "@/hooks/useImageDrop";
import { getMonacoLanguage, isBinaryFormat } from "@/lib/formats";

interface EmptyStateProps {
  icon: React.ReactNode;
  title: string;
  description: string;
  action?: React.ReactNode;
}

function EmptyState({ icon, title, description, action }: EmptyStateProps) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-3 px-6 text-center">
      <div className="rounded-lg border border-zinc-800 bg-zinc-900/50 p-3 text-zinc-600">
        {icon}
      </div>
      <div className="space-y-1">
        <p className="text-sm font-medium text-zinc-400">{title}</p>
        <p className="text-xs text-zinc-600">{description}</p>
      </div>
      {action}
    </div>
  );
}

function DropOverlay() {
  return (
    <div className="absolute inset-0 z-10 flex items-center justify-center bg-zinc-950/80 backdrop-blur-sm">
      <div className="flex flex-col items-center gap-2 rounded-xl border-2 border-dashed border-blue-500 px-10 py-8">
        <FileUp className="h-8 w-8 text-blue-400" />
        <p className="text-sm font-medium text-blue-300">Drop file</p>
      </div>
    </div>
  );
}

function DocxButton({ onClick }: { onClick: () => void }) {
  return (
    <button
      className="mt-2 inline-flex items-center gap-1.5 rounded-md bg-zinc-800 px-3 py-1.5 text-xs text-zinc-300 hover:bg-zinc-700 transition-colors"
      onClick={onClick}
    >
      <FileUp className="h-3.5 w-3.5" />
      Open .docx
    </button>
  );
}

export function Editor() {
  const { activeWorkspaceId, activeFileId } = useWorkspace();
  const { content, filePath, onChange } = useActiveFile(activeFileId);
  const { importDocxFile, openFilePicker } = useDocxImport();
  const editorRef = useRef<monacoEditor.IStandaloneCodeEditor | null>(null);
  const handleImageDrop = useImageDrop();

  const onImage = useCallback(
    async (file: File) => {
      const filename = await handleImageDrop(file);
      if (!filename || !editorRef.current) return;
      const ed = editorRef.current;
      const pos = ed.getPosition();
      if (pos) {
        const text = `![](${filename})`;
        ed.executeEdits("image-drop", [
          {
            range: {
              startLineNumber: pos.lineNumber,
              startColumn: pos.column,
              endLineNumber: pos.lineNumber,
              endColumn: pos.column,
            },
            text,
          },
        ]);
      }
    },
    [handleImageDrop],
  );

  const { isDragging, dropProps } = useDropZone(importDocxFile, onImage);

  const isBinary = filePath ? isBinaryFormat(filePath) : false;

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
      <div className="relative h-full" {...dropProps}>
        {isDragging && <DropOverlay />}
        <EmptyState
          icon={<FileText className="h-5 w-5" />}
          title="No file selected"
          description="Pick a file from the sidebar, or drop a .docx here"
          action={<DocxButton onClick={openFilePicker} />}
        />
      </div>
    );
  }

  if (isBinary) {
    return (
      <div className="relative h-full" {...dropProps}>
        {isDragging && <DropOverlay />}
        <EmptyState
          icon={<FileText className="h-5 w-5" />}
          title={filePath}
          description="Binary file — check Preview and AST tabs for output"
          action={<DocxButton onClick={openFilePicker} />}
        />
      </div>
    );
  }

  return (
    <div className="relative h-full" {...dropProps}>
      {isDragging && <DropOverlay />}
      <MonacoEditor
        onMount={(editor) => { editorRef.current = editor; }}
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
    </div>
  );
}
