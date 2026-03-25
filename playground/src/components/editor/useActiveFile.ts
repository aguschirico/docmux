import { useEffect, useRef, useState, useCallback } from "react";
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
  const lastLoadedId = useRef<number | null>(null);

  // Sync local content when file changes (new file selected or external update)
  useEffect(() => {
    if (file && file.id !== lastLoadedId.current) {
      setLocalContent(file.content);
      lastLoadedId.current = file.id ?? null;
    }
  }, [file]);

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
