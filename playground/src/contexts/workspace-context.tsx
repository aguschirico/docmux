import { createContext, useContext, useState, type ReactNode } from "react";

interface WorkspaceState {
  activeWorkspaceId: number | null;
  activeFileId: number | null;
  setActiveWorkspaceId: (id: number | null) => void;
  setActiveFileId: (id: number | null) => void;
}

const WorkspaceContext = createContext<WorkspaceState | null>(null);

export function WorkspaceProvider({ children }: { children: ReactNode }) {
  const [activeWorkspaceId, setActiveWorkspaceId] = useState<number | null>(
    null,
  );
  const [activeFileId, setActiveFileId] = useState<number | null>(null);

  return (
    <WorkspaceContext.Provider
      value={{
        activeWorkspaceId,
        activeFileId,
        setActiveWorkspaceId,
        setActiveFileId,
      }}
    >
      {children}
    </WorkspaceContext.Provider>
  );
}

export function useWorkspace(): WorkspaceState {
  const ctx = useContext(WorkspaceContext);
  if (!ctx) {
    throw new Error("useWorkspace must be used within WorkspaceProvider");
  }
  return ctx;
}
