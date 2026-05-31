import * as React from "react";
import { ScrollArea, ScrollBar } from "@/components/ui/scroll-area";

type ForgeScrollAreaProps = React.ComponentPropsWithoutRef<typeof ScrollArea>;
type ForgeScrollAreaRef = React.ComponentRef<typeof ScrollArea>;

const ForgeScrollArea = React.forwardRef<ForgeScrollAreaRef, ForgeScrollAreaProps>(function ForgeScrollArea(props, ref) {
  return React.createElement(ScrollArea, {
    ...props,
    ref,
  });
});

ForgeScrollArea.displayName = "ForgeScrollArea";

export { ScrollArea, ScrollBar, ForgeScrollArea };
