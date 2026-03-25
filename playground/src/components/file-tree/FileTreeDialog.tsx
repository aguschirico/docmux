import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type { TreeNode } from "@/lib/file-tree";

export type DialogState =
  | { type: "closed" }
  | { type: "new-file"; folderPath: string }
  | { type: "new-folder"; folderPath: string }
  | { type: "rename"; node: TreeNode };

interface FileTreeDialogProps {
  dialog: DialogState;
  inputValue: string;
  onInputChange: (value: string) => void;
  onSubmit: () => void;
  onClose: () => void;
}

export function FileTreeDialog({
  dialog,
  inputValue,
  onInputChange,
  onSubmit,
  onClose,
}: FileTreeDialogProps) {
  const title =
    dialog.type === "new-file"
      ? "New File"
      : dialog.type === "new-folder"
        ? "New Folder"
        : "Rename";

  return (
    <Dialog
      open={dialog.type !== "closed"}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <DialogContent className="sm:max-w-sm">
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
        </DialogHeader>
        <Input
          value={inputValue}
          onChange={(e) => onInputChange(e.target.value)}
          placeholder={
            dialog.type === "new-folder" ? "folder-name" : "filename.md"
          }
          onKeyDown={(e) => {
            if (e.key === "Enter") onSubmit();
          }}
          autoFocus
        />
        <DialogFooter>
          <Button size="sm" onClick={onSubmit}>
            {dialog.type === "rename" ? "Rename" : "Create"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
