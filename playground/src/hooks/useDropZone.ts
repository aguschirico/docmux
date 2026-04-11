import { useState, useCallback, type DragEvent } from "react";

const IMAGE_EXTENSIONS = new Set(["png", "jpg", "jpeg", "gif", "webp"]);

function getFileExtension(name: string): string {
  const dot = name.lastIndexOf(".");
  return dot >= 0 ? name.slice(dot + 1).toLowerCase() : "";
}

export function isImageFile(file: File): boolean {
  return IMAGE_EXTENSIONS.has(getFileExtension(file.name));
}

export function useDropZone(onFile: (file: File) => void, onImage?: (file: File) => void) {
  const [dragCount, setDragCount] = useState(0);

  const handleDragEnter = useCallback((e: DragEvent) => {
    e.preventDefault();
    setDragCount((n) => n + 1);
  }, []);

  const handleDragLeave = useCallback((e: DragEvent) => {
    e.preventDefault();
    setDragCount((n) => n - 1);
  }, []);

  const handleDragOver = useCallback((e: DragEvent) => {
    e.preventDefault();
  }, []);

  const handleDrop = useCallback(
    (e: DragEvent) => {
      e.preventDefault();
      setDragCount(0);
      const file = e.dataTransfer.files[0];
      if (!file) return;
      if (onImage && isImageFile(file)) {
        onImage(file);
      } else {
        onFile(file);
      }
    },
    [onFile, onImage],
  );

  return {
    isDragging: dragCount > 0,
    dropProps: {
      onDragEnter: handleDragEnter,
      onDragLeave: handleDragLeave,
      onDragOver: handleDragOver,
      onDrop: handleDrop,
    },
  };
}
