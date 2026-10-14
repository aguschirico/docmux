import { DocxEditor } from "@docx-editor.dev/react";
import "@docx-editor.dev/core/styles/editor.css";

interface WordPreviewProps {
  bytes: Uint8Array;
}

/** A .docx laid out as Word pages, read-only. Loaded on demand: it is large. */
export default function WordPreview({ bytes }: WordPreviewProps) {
  return (
    <div className="h-full">
      <DocxEditor document={bytes} mode="view" chrome={false} colorMode="dark" />
    </div>
  );
}
