import { useEffect, useRef, useState } from "react";
import { renderAsync } from "docx-preview";
import { adaptMath, fixRenderedMath } from "@/lib/docx-preview-math";

interface WordPreviewProps {
  bytes: Uint8Array;
}

/** Horizontal padding of `.docx-wrapper`, from index.css. */
const WRAPPER_PADDING = 32;

/** Scales the pages down to the pane on narrow screens: a Word page is wider than a phone. */
function fitPages(frame: HTMLElement) {
  const wrapper = frame.querySelector<HTMLElement>(".docx-wrapper");
  const page = wrapper?.querySelector<HTMLElement>("section.docx");
  if (!wrapper || !page) return;
  const pageWidth = parseFloat(getComputedStyle(page).width);
  if (!pageWidth) return;
  wrapper.style.zoom = String(Math.min(1, (frame.clientWidth - WRAPPER_PADDING) / pageWidth));
}

/**
 * A .docx laid out as pages, read-only. docx-preview renders Word math
 * (OMML) as MathML, which browsers draw natively. Loaded on demand.
 */
export default function WordPreview({ bytes }: WordPreviewProps) {
  const frame = useRef<HTMLDivElement>(null);
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
        if (frame.current) fitPages(frame.current);
        setError(null);
      })
      .catch((err: unknown) => {
        if (!cancelled) setError(String(err));
      });
    return () => {
      cancelled = true;
    };
  }, [bytes]);

  useEffect(() => {
    const el = frame.current;
    if (!el) return;
    const observer = new ResizeObserver(() => fitPages(el));
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  return (
    <div ref={frame} className="word-preview h-full overflow-auto bg-zinc-900">
      <div ref={styles} />
      {error && <p className="p-6 text-sm text-red-400">Could not display this document: {error}</p>}
      <div ref={body} hidden={error !== null} />
    </div>
  );
}
