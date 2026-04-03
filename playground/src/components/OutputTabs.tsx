import { useState } from "react";
import { useLiveQuery } from "dexie-react-hooks";
import { Download } from "lucide-react";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useWorkspace } from "@/contexts/workspace-context";
import { useConversion } from "@/hooks/useConversion";
import { db } from "@/vfs/db";
import { getFormat, getExtension, isBinaryFormat } from "@/lib/formats";
import { HtmlPreview } from "@/components/output-tabs/HtmlPreview";
import { ReadOnlyEditor } from "@/components/output-tabs/ReadOnlyEditor";
import { DiagnosticsView } from "@/components/output-tabs/DiagnosticsView";

const OUTPUT_FORMATS = [
  { value: "html", label: "HTML" },
  { value: "latex", label: "LaTeX" },
  { value: "typst", label: "Typst" },
  { value: "markdown", label: "Markdown" },
  { value: "plain", label: "Plain Text" },
] as const;

const FORMAT_TO_MONACO: Record<string, string> = {
  html: "html",
  latex: "latex",
  typst: "plaintext",
  markdown: "markdown",
  plain: "plaintext",
};

const FORMAT_TO_EXT: Record<string, string> = {
  html: "html",
  latex: "tex",
  typst: "typ",
  markdown: "md",
  plain: "txt",
};

export function OutputTabs() {
  const { activeFileId } = useWorkspace();
  const [outputFormat, setOutputFormat] = useState("html");

  const file = useLiveQuery(
    () => (activeFileId ? db.files.get(activeFileId) : undefined),
    [activeFileId],
  );

  const inputFormat = file?.path ? getFormat(file.path) : null;
  const isBinary = inputFormat ? isBinaryFormat(inputFormat) : false;
  const content = isBinary ? null : (file?.content ?? null);
  const binaryContent = isBinary && file?.binaryContent
    ? new Uint8Array(file.binaryContent)
    : null;

  const { preview, source, ast, errors, converting } = useConversion(
    content,
    inputFormat,
    outputFormat,
    binaryContent,
  );

  function handleDownload() {
    if (!source) return;
    const ext = FORMAT_TO_EXT[outputFormat] ?? "txt";
    const baseName = file?.path
      ? file.path.replace(`.${getExtension(file.path)}`, "")
      : "output";
    const blob = new Blob([source], { type: "text/plain;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `${baseName}.${ext}`;
    a.click();
    URL.revokeObjectURL(url);
  }

  return (
    <Tabs defaultValue="preview" className="flex h-full flex-col">
      <div className="flex items-center gap-2 border-b border-zinc-800 px-3 py-1.5">
        <TabsList variant="line" className="h-7 gap-0">
          <TabsTrigger value="preview" className="px-2 text-xs">
            Preview
            <span className="ml-1 text-[10px] text-zinc-500">HTML</span>
          </TabsTrigger>
          <TabsTrigger value="source" className="px-2 text-xs">
            Source
          </TabsTrigger>
          <div className="ml-0.5">
            <Select value={outputFormat} onValueChange={setOutputFormat}>
              <SelectTrigger className="h-5 w-24 border-zinc-700 bg-zinc-900 text-[10px]">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {OUTPUT_FORMATS.map((f) => (
                  <SelectItem key={f.value} value={f.value} className="text-xs">
                    {f.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <TabsTrigger value="ast" className="px-2 text-xs">
            AST
          </TabsTrigger>
          <TabsTrigger value="diagnostics" className="px-2 text-xs">
            Diagnostics
            {errors.length > 0 && (
              <span className="ml-1 rounded-full bg-red-900/60 px-1.5 text-[10px] text-red-300">
                {errors.length}
              </span>
            )}
          </TabsTrigger>
        </TabsList>

        <div className="ml-auto" />
        <button
          disabled={!source}
          onClick={handleDownload}
          className="inline-flex h-6 items-center gap-1 rounded px-1.5 text-[11px] text-zinc-400 transition-colors hover:bg-zinc-800 hover:text-zinc-100 disabled:pointer-events-none disabled:opacity-30"
          title="Download converted output"
        >
          <Download className="h-3 w-3" />
          Export
        </button>
      </div>

      <TabsContent value="preview" className="flex-1 overflow-auto">
        <HtmlPreview html={preview} />
      </TabsContent>
      <TabsContent value="source" className="flex-1 overflow-auto">
        <ReadOnlyEditor
          value={source}
          language={FORMAT_TO_MONACO[outputFormat] ?? "plaintext"}
          emptyMessage="Select a file and output format"
        />
      </TabsContent>
      <TabsContent value="ast" className="flex-1 overflow-auto">
        <ReadOnlyEditor
          value={ast}
          language="json"
          emptyMessage="Select a file to inspect its AST"
        />
      </TabsContent>
      <TabsContent value="diagnostics" className="flex-1 overflow-auto">
        <DiagnosticsView errors={errors} converting={converting} />
      </TabsContent>
    </Tabs>
  );
}
