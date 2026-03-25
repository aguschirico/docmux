import MonacoEditor from "@monaco-editor/react";

interface ReadOnlyEditorProps {
  value: string | null;
  language: string;
  emptyMessage?: string;
}

export function ReadOnlyEditor({
  value,
  language,
  emptyMessage = "No output",
}: ReadOnlyEditorProps) {
  if (!value) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-zinc-600">
        {emptyMessage}
      </div>
    );
  }

  return (
    <MonacoEditor
      height="100%"
      theme="vs-dark"
      language={language}
      value={value}
      options={{
        readOnly: true,
        minimap: { enabled: false },
        fontSize: 12,
        fontFamily: "JetBrains Mono, Fira Code, monospace",
        lineNumbers: "on",
        scrollBeyondLastLine: false,
        wordWrap: "on",
        padding: { top: 12 },
        renderLineHighlight: "none",
        domReadOnly: true,
      }}
    />
  );
}
