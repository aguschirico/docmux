import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";

function Placeholder({ label }: { label: string }) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-2 text-zinc-600">
      <span className="text-sm font-medium">{label}</span>
      <span className="text-xs">Connect docmux WASM to enable conversion</span>
    </div>
  );
}

export function OutputTabs() {
  return (
    <Tabs defaultValue="preview" className="flex h-full flex-col">
      <div className="flex items-center border-b border-zinc-800 px-3 py-1.5">
        <TabsList variant="line" className="h-7 gap-0">
          <TabsTrigger value="preview" className="px-2 text-xs">
            Preview
          </TabsTrigger>
          <TabsTrigger value="source" className="px-2 text-xs">
            Source
          </TabsTrigger>
          <TabsTrigger value="ast" className="px-2 text-xs">
            AST
          </TabsTrigger>
          <TabsTrigger value="diagnostics" className="px-2 text-xs">
            Diagnostics
          </TabsTrigger>
        </TabsList>
      </div>

      <TabsContent value="preview" className="flex-1 overflow-auto">
        <Placeholder label="Preview" />
      </TabsContent>
      <TabsContent value="source" className="flex-1 overflow-auto">
        <Placeholder label="Source Output" />
      </TabsContent>
      <TabsContent value="ast" className="flex-1 overflow-auto">
        <Placeholder label="AST Inspector" />
      </TabsContent>
      <TabsContent value="diagnostics" className="flex-1 overflow-auto">
        <Placeholder label="Diagnostics" />
      </TabsContent>
    </Tabs>
  );
}
