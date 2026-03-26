import { useRef, useState, useCallback } from "react";
import { useLiveQuery } from "dexie-react-hooks";
import { db, updateFileContent } from "@/vfs/db";
import { debounce } from "@/lib/debounce";

const SAVE_DELAY_MS = 400;

export function useActiveFile(fileId: number | null) {
  const file = useLiveQuery(
    () => (fileId ? db.files.get(fileId) : undefined),
    [fileId],
  );

  const [localContent, setLocalContent] = useState("");
  const [loadedFileId, setLoadedFileId] = useState<number | null>(null);

  // Render-time state adjustment: sync content when a different file is selected.
  // See https://react.dev/learn/you-might-not-need-an-effect#adjusting-some-state-when-a-prop-changes
  if (file && file.id !== loadedFileId) {
    setLocalContent(file.content);
    setLoadedFileId(file.id ?? null);
  }

  const debouncedSave = useRef(
    debounce((id: number, content: string) => {
      updateFileContent(id, content);
    }, SAVE_DELAY_MS),
  ).current;

  const handleChange = useCallback(
    (value: string | undefined) => {
      const content = value ?? "";
      setLocalContent(content);
      if (fileId) {
        debouncedSave(fileId, content);
      }
    },
    [fileId, debouncedSave],
  );

  return {
    content: localContent,
    filePath: file?.path ?? null,
    onChange: handleChange,
  };
}
