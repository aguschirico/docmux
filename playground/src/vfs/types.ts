export interface VfsWorkspace {
  id?: number;
  name: string;
  createdAt: Date;
}

export interface VfsFile {
  id?: number;
  workspaceId: number;
  path: string;
  content: string;
  updatedAt: Date;
}
