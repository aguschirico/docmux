import { lazy, Suspense } from "react";

const WordPreview = lazy(() => import("@/components/output-tabs/WordPreview"));

/** The Word page view, loaded the first time a .docx is shown. */
export function WordView({ bytes }: { bytes: Uint8Array }) {
  return (
    <Suspense
      fallback={
        <div className="flex h-full items-center justify-center text-sm text-zinc-500">
          Loading the Word viewer…
        </div>
      }
    >
      <WordPreview bytes={bytes} />
    </Suspense>
  );
}
