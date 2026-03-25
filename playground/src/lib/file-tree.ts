export interface TreeNode {
  name: string;
  path: string;
  isFolder: boolean;
  fileId?: number;
  children: TreeNode[];
}

export function buildTree(
  files: Array<{ id?: number; path: string }>,
): TreeNode[] {
  const root: TreeNode[] = [];

  for (const file of files) {
    const parts = file.path.split("/");
    let current = root;

    for (let i = 0; i < parts.length; i++) {
      const name = parts[i]!;
      const isLast = i === parts.length - 1;
      const partPath = parts.slice(0, i + 1).join("/");

      let existing = current.find((n) => n.name === name);

      if (!existing) {
        existing = {
          name,
          path: partPath,
          isFolder: !isLast,
          fileId: isLast ? file.id : undefined,
          children: [],
        };
        current.push(existing);
      }

      if (!isLast) {
        current = existing.children;
      }
    }
  }

  return sortNodes(root);
}

function sortNodes(nodes: TreeNode[]): TreeNode[] {
  nodes.sort((a, b) => {
    if (a.isFolder !== b.isFolder) return a.isFolder ? -1 : 1;
    return a.name.localeCompare(b.name);
  });
  for (const node of nodes) {
    sortNodes(node.children);
  }
  return nodes;
}
