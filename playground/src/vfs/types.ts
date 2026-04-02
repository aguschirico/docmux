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
  /** Raw bytes for binary formats (e.g. DOCX). When set, `content` is empty. */
  binaryContent?: ArrayBuffer;
  updatedAt: Date;
}
