import { useState } from "react";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { InstalledTab } from "./InstalledTab";
import { DiscoverTab } from "./DiscoverTab";

export function PluginManagerPanel() {
  const [tab, setTab] = useState("installed");

  return (
    <div className="flex flex-col h-full min-h-0 overflow-hidden px-3 py-2">
      <Tabs value={tab} onValueChange={setTab} className="flex flex-col flex-1">
        <TabsList className="w-full grid grid-cols-2 mb-2 shrink-0">
          <TabsTrigger value="installed" className="text-xs">
            Installed
          </TabsTrigger>
          <TabsTrigger value="discover" className="text-xs">
            Discover
          </TabsTrigger>
        </TabsList>

        <TabsContent value="installed" className="flex-1 overflow-auto mt-0">
          <InstalledTab />
        </TabsContent>

        <TabsContent value="discover" className="flex-1 overflow-auto mt-0">
          <DiscoverTab />
        </TabsContent>
      </Tabs>
    </div>
  );
}
