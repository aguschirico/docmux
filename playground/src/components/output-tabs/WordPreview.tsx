import { useEffect, useRef, useState } from "react";
import { renderAsync } from "docx-preview";
import { adaptMath, fixRenderedMath } from "@/lib/docx-preview-math";

interface WordPreviewProps {
  bytes: Uint8Array;
}

/**
 * A .docx laid out as pages, read-only. docx-preview renders Word math
 * (OMML) as MathML, which browsers draw natively. Loaded on demand.
 */
export default function WordPreview({ bytes }: WordPreviewProps) {
  const body = useRef<HTMLDivElement>(null);
  const styles = useRef<HTMLDivElement>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const target = body.current;
    if (!target) return;
    let cancelled = false;
    adaptMath(bytes)
      .then((adapted) =>
        renderAsync(adapted, target, styles.current ?? undefined, {
          className: "docx",
          inWrapper: true,
          breakPages: true,
          ignoreLastRenderedPageBreak: true,
          renderFootnotes: true,
          useBase64URL: true,
        }),
      )
      .then(() => {
        if (cancelled) return;
        fixRenderedMath(target);
        setError(null);
      })
      .catch((err: unknown) => {
        if (!cancelled) setError(String(err));
      });
    return () => {
      cancelled = true;
    };
  }, [bytes]);

  return (
    <div className="word-preview h-full overflow-auto bg-zinc-900">
      <div ref={styles} />
      {error && <p className="p-6 text-sm text-red-400">Could not display this document: {error}</p>}
      <div ref={body} hidden={error !== null} />
    </div>
  );
}
