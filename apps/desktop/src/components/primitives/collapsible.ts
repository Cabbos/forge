import * as React from "react";
import {
  Collapsible,
  CollapsibleTrigger,
  CollapsibleContent,
} from "@/components/ui/collapsible";

type ForgeCollapsibleRef = React.ComponentRef<typeof Collapsible>;
type ForgeCollapsibleProps = React.ComponentPropsWithoutRef<typeof Collapsible>;
type ForgeCollapsibleTriggerRef = React.ComponentRef<typeof CollapsibleTrigger>;
type ForgeCollapsibleTriggerProps = React.ComponentPropsWithoutRef<typeof CollapsibleTrigger>;
type ForgeCollapsibleContentRef = React.ComponentRef<typeof CollapsibleContent>;
type ForgeCollapsibleContentProps = React.ComponentPropsWithoutRef<typeof CollapsibleContent>;

const ForgeCollapsible = React.forwardRef<ForgeCollapsibleRef, ForgeCollapsibleProps>(function ForgeCollapsible(props, ref) {
  return React.createElement(Collapsible, { ...props, ref });
});

const ForgeCollapsibleTrigger = React.forwardRef<ForgeCollapsibleTriggerRef, ForgeCollapsibleTriggerProps>(
  function ForgeCollapsibleTrigger(props, ref) {
    return React.createElement(CollapsibleTrigger, { ...props, ref });
  },
);

const ForgeCollapsibleContent = React.forwardRef<ForgeCollapsibleContentRef, ForgeCollapsibleContentProps>(
  function ForgeCollapsibleContent(props, ref) {
    return React.createElement(CollapsibleContent, { ...props, ref });
  },
);

ForgeCollapsible.displayName = "ForgeCollapsible";
ForgeCollapsibleTrigger.displayName = "ForgeCollapsibleTrigger";
ForgeCollapsibleContent.displayName = "ForgeCollapsibleContent";

export {
  Collapsible,
  CollapsibleTrigger,
  CollapsibleContent,
  ForgeCollapsible,
  ForgeCollapsibleTrigger,
  ForgeCollapsibleContent,
};
