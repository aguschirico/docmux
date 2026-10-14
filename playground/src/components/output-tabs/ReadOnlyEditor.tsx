import MonacoEditor from "@monaco-editor/react";
import { EDITOR_OPTIONS, MONACO_THEME, setupMonaco } from "@/lib/monaco";

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
      theme={MONACO_THEME}
      beforeMount={setupMonaco}
      loading={null}
      language={language}
      value={value}
      options={{
        ...EDITOR_OPTIONS,
        readOnly: true,
        fontSize: 13,
        renderLineHighlight: "none",
        domReadOnly: true,
      }}
    />
  );
}
