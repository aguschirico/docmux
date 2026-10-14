import { useMemo, useState } from "react";
import { useLiveQuery } from "dexie-react-hooks";
import { Check, ChevronDown, Copy, Download } from "lucide-react";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useWorkspace } from "@/contexts/workspace-context";
import { db } from "@/vfs/db";
import { getFormat, isBinaryFormat } from "@/lib/formats";
import {
  MAIN_TABS,
  MORE_TABS,
  PREVIEW_TAB,
  findTab,
  formatName,
  type OutputTab,
} from "@/lib/output-formats";
import { useConversion, type ConversionInput, type ConversionState } from "@/hooks/useConversion";
import { HtmlPreview } from "@/components/output-tabs/HtmlPreview";
import { ReadOnlyEditor } from "@/components/output-tabs/ReadOnlyEditor";
import { WordView } from "@/components/output-tabs/WordView";

const IMAGE = /\.(png|jpe?g|gif|webp|svg)$/i;
const encoder = new TextEncoder();

/** The active file plus the rest of its workspace, as conversion input. */
function useConversionInput(): ConversionInput {
  const { activeWorkspaceId, activeFileId } = useWorkspace();
  const files = useLiveQuery(
    () =>
      activeWorkspaceId ? db.files.where("workspaceId").equals(activeWorkspaceId).toArray() : [],
    [activeWorkspaceId],
  );
  return useMemo(() => {
    const active = files?.find((f) => f.id === activeFileId);
    const projectFiles = new Map<string, Uint8Array>();
    const resources = new Map<string, Uint8Array>();
    for (const f of files ?? []) {
      if (f.binaryContent && IMAGE.test(f.path)) {
        resources.set(f.path, new Uint8Array(f.binaryContent));
      } else if (!f.binaryContent && f.id !== activeFileId) {
        projectFiles.set(f.path, encoder.encode(f.content));
      }
    }
    const binary = active && isBinaryFormat(active.path) && active.binaryContent;
    return {
      path: active?.path ?? null,
      content: active && !binary ? active.content : null,
      binary: binary ? new Uint8Array(binary) : null,
      projectFiles,
      resources,
    };
  }, [files, activeFileId]);
}

function TabButton({ tab, active, onSelect }: { tab: OutputTab; active: boolean; onSelect: () => void }) {
  return (
    <button
      onClick={onSelect}
      className={`relative h-11 shrink-0 px-3.5 text-sm font-medium transition-colors ${
        active ? "text-zinc-50" : "text-zinc-400 hover:text-zinc-200"
      }`}
    >
      {tab.label}
      {active && <span className="absolute inset-x-2.5 bottom-0 h-0.5 rounded-full bg-sky-400 shadow-[0_0_8px] shadow-sky-400/70" />}
    </button>
  );
}

function download(data: BlobPart, name: string, type: string) {
  const url = URL.createObjectURL(new Blob([data], { type }));
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.click();
  URL.revokeObjectURL(url);
}

function Actions({ tab, state, baseName }: { tab: OutputTab; state: ConversionState; baseName: string }) {
  const [copied, setCopied] = useState(false);
  const text = tab.id === PREVIEW_TAB ? state.preview : state.source;
  const name = `${baseName}.${tab.extension}`;

  async function copy() {
    if (!text) return;
    await navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 1200);
  }

  function save() {
    if (state.binaryOutput) {
      download(new Uint8Array(state.binaryOutput), name, "application/vnd.openxmlformats-officedocument.wordprocessingml.document");
    } else if (text) {
      download(text, name, "text/plain;charset=utf-8");
    }
  }

  const button =
    "inline-flex h-7 items-center gap-1.5 rounded-md px-2 text-xs text-zinc-400 transition-colors hover:bg-zinc-800 hover:text-zinc-100 disabled:pointer-events-none disabled:opacity-30";
  return (
    <div className="ml-auto flex shrink-0 items-center gap-1 pl-2">
      {!state.binaryOutput && (
        <button className={button} onClick={copy} disabled={!text} title="Copy">
          {copied ? <Check className="h-3.5 w-3.5" /> : <Copy className="h-3.5 w-3.5" />}
          <span className="hidden sm:inline">{copied ? "Copied" : "Copy"}</span>
        </button>
      )}
      <button className={button} onClick={save} disabled={!text && !state.binaryOutput} title="Save">
        <Download className="h-3.5 w-3.5" />
        <span className="hidden sm:inline">Save</span>
      </button>
    </div>
  );
}

function Centered({ children }: { children: string }) {
  return <div className="flex h-full items-center justify-center text-sm text-zinc-500">{children}</div>;
}

function Body({ tab, state }: { tab: OutputTab; state: ConversionState }) {
  if (tab.id === PREVIEW_TAB) return <HtmlPreview html={state.preview} />;
  if (isBinaryFormat(tab.id)) {
    if (!state.binaryOutput) {
      return <Centered>{state.errors.length > 0 ? "Could not convert: see the error below." : "Converting…"}</Centered>;
    }
    return <WordView bytes={state.binaryOutput} />;
  }
  return <ReadOnlyEditor value={state.source} language={tab.language} emptyMessage="Converting…" />;
}

function StatusLine({ tab, state }: { tab: OutputTab; state: ConversionState }) {
  if (!state.inputFormat) return null;
  const target = tab.id === PREVIEW_TAB ? "HTML preview" : tab.label;
  const failed = state.errors.length > 0;
  return (
    <div className="flex shrink-0 items-center gap-3 border-t border-zinc-800/80 bg-zinc-900/40 px-4 py-2 text-xs text-zinc-400">
      <span className="inline-flex items-center gap-2">
        <span className={`h-1.5 w-1.5 rounded-full ${failed ? "bg-red-400" : "bg-emerald-400"}`} />
        <span className="text-zinc-200">{formatName(state.inputFormat)}</span>
        <span className="text-zinc-600">→</span>
        <span className="text-zinc-200">{target}</span>
      </span>
      {state.ms !== null && (
        <span className="rounded bg-zinc-800 px-1.5 py-0.5 font-mono text-[11px] text-zinc-300">
          {state.ms} ms
        </span>
      )}
      {failed ? (
        <span className="truncate text-red-400" title={state.errors.join("\n")}>
          {state.errors.length} error{state.errors.length > 1 ? "s" : ""}: {state.errors[0]}
        </span>
      ) : (
        <span className="ml-auto hidden text-zinc-500 sm:inline">Converted locally, in your browser</span>
      )}
    </div>
  );
}

interface OutputPanelProps {
  tabId: string;
  onTabChange: (id: string) => void;
}

export function OutputPanel({ tabId, onTabChange }: OutputPanelProps) {
  const input = useConversionInput();
  const tab = findTab(tabId);
  const state = useConversion(input, tab.id);
  const baseName = input.path ? input.path.replace(/\.[^.]+$/, "") : "output";
  const inputFormat = input.path ? getFormat(input.path) : null;
  const moreActive = MORE_TABS.some((t) => t.id === tab.id);

  return (
    <div className="flex h-full flex-col">
      <div className="flex shrink-0 items-center border-b border-zinc-800/80 bg-zinc-900/40 pl-2 pr-2">
        <div className="flex min-w-0 items-center overflow-x-auto">
          {MAIN_TABS.filter((t) => t.id !== inputFormat).map((t) => (
            <TabButton key={t.id} tab={t} active={t.id === tab.id} onSelect={() => onTabChange(t.id)} />
          ))}
          <DropdownMenu>
            <DropdownMenuTrigger
              className={`inline-flex h-11 shrink-0 items-center gap-1 px-3.5 text-sm font-medium transition-colors ${
                moreActive ? "text-zinc-50" : "text-zinc-400 hover:text-zinc-200"
              }`}
            >
              {moreActive ? tab.label : "More"}
              <ChevronDown className="h-3.5 w-3.5" />
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start">
              {MORE_TABS.map((t) => (
                <DropdownMenuItem key={t.id} onClick={() => onTabChange(t.id)}>
                  {t.label}
                </DropdownMenuItem>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
        <Actions tab={tab} state={state} baseName={baseName} />
      </div>
      <div className="min-h-0 flex-1">
        <Body tab={tab} state={state} />
      </div>
      <StatusLine tab={tab} state={state} />
    </div>
  );
}
