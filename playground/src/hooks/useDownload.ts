import { isBinaryFormat } from "@/lib/formats";

const FORMAT_TO_EXT: Record<string, string> = {
  html: "html",
  latex: "tex",
  typst: "typ",
  markdown: "md",
  plain: "txt",
  docx: "docx",
};

function getExtensionFromPath(path: string): string {
  const dot = path.lastIndexOf(".");
  return dot !== -1 ? path.slice(dot + 1) : "";
}

function triggerBlobDownload(blob: Blob, filename: string) {
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}

export function useDownload(
  outputFormat: string,
  filePath: string | undefined,
  source: string | null,
  binaryOutput: Uint8Array | null,
) {
  const ext = FORMAT_TO_EXT[outputFormat] ?? "txt";
  const baseName = filePath
    ? filePath.replace(`.${getExtensionFromPath(filePath)}`, "")
    : "output";

  function handleDownload() {
    if (isBinaryFormat(outputFormat) && binaryOutput) {
      const blob = new Blob([binaryOutput], {
        type: "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
      });
      triggerBlobDownload(blob, `${baseName}.${ext}`);
    } else if (source) {
      const blob = new Blob([source], { type: "text/plain;charset=utf-8" });
      triggerBlobDownload(blob, `${baseName}.${ext}`);
    }
  }

  const canDownload = isBinaryFormat(outputFormat) ? binaryOutput !== null : source !== null;

  return { handleDownload, canDownload };
}
