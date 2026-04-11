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
import { getFormat, isBinaryFormat } from "@/lib/formats";
import { useDownload } from "@/hooks/useDownload";
import { HtmlPreview } from "@/components/output-tabs/HtmlPreview";
import { ReadOnlyEditor } from "@/components/output-tabs/ReadOnlyEditor";
import { DiagnosticsView } from "@/components/output-tabs/DiagnosticsView";

const OUTPUT_FORMATS = [
  { value: "html", label: "HTML" },
  { value: "latex", label: "LaTeX" },
  { value: "typst", label: "Typst" },
  { value: "markdown", label: "Markdown" },
  { value: "plain", label: "Plain Text" },
  { value: "docx", label: "DOCX" },
] as const;

const FORMAT_TO_MONACO: Record<string, string> = {
  html: "html",
  latex: "latex",
  typst: "plaintext",
  markdown: "markdown",
  plain: "plaintext",
  docx: "plaintext",
};

export function OutputTabs() {
  const { activeFileId, activeWorkspaceId } = useWorkspace();
  const [outputFormat, setOutputFormat] = useState("html");

  const file = useLiveQuery(
    () => (activeFileId ? db.files.get(activeFileId) : undefined),
    [activeFileId],
  );

  const imageResources = useLiveQuery(
    async () => {
      if (!activeWorkspaceId) return null;
      const files = await db.files
        .where("workspaceId")
        .equals(activeWorkspaceId)
        .toArray();
      const map = new Map<string, Uint8Array>();
      for (const f of files) {
        if (f.binaryContent && /\.(png|jpe?g|gif|webp)$/i.test(f.path)) {
          map.set(f.path, new Uint8Array(f.binaryContent));
        }
      }
      return map.size > 0 ? map : null;
    },
    [activeWorkspaceId],
  );

  const inputFormat = file?.path ? getFormat(file.path) : null;
  const isBinary = inputFormat ? isBinaryFormat(inputFormat) : false;
  const content = isBinary ? null : (file?.content ?? null);
  const binaryContent = isBinary && file?.binaryContent
    ? new Uint8Array(file.binaryContent)
    : null;

  const { preview, source, binaryOutput, ast, errors, converting } = useConversion(
    content,
    inputFormat,
    outputFormat,
    binaryContent,
    imageResources,
  );

  const { handleDownload, canDownload } = useDownload(
    outputFormat,
    file?.path,
    source,
    binaryOutput,
  );

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
            <Select value={outputFormat} onValueChange={(v) => { if (v !== null) setOutputFormat(v); }}>
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
          disabled={!canDownload}
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
        {isBinaryFormat(outputFormat) ? (
          <div className="flex h-full items-center justify-center text-sm text-zinc-500">
            Binary format — use Export to download
          </div>
        ) : (
          <ReadOnlyEditor
            value={source}
            language={FORMAT_TO_MONACO[outputFormat] ?? "plaintext"}
            emptyMessage="Select a file and output format"
          />
        )}
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
