import { CircleAlert, CircleCheck } from "lucide-react";

interface DiagnosticsViewProps {
  errors: string[];
  converting: boolean;
}

export function DiagnosticsView({ errors, converting }: DiagnosticsViewProps) {
  if (converting) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-zinc-500">
        Converting…
      </div>
    );
  }

  if (errors.length === 0) {
    return (
      <div className="flex h-full items-center justify-center gap-2 text-sm text-emerald-500">
        <CircleCheck className="size-4" />
        No errors
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-2 p-3">
      {errors.map((err, i) => (
        <div
          key={i}
          className="flex items-start gap-2 rounded border border-red-900/50 bg-red-950/30 px-3 py-2 text-xs text-red-400"
        >
          <CircleAlert className="mt-0.5 size-3.5 shrink-0" />
          <span className="font-mono whitespace-pre-wrap">{err}</span>
        </div>
      ))}
    </div>
  );
}
