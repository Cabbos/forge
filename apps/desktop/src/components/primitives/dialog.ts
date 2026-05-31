import * as React from "react";
import {
  Dialog,
  DialogPortal,
  DialogOverlay,
  DialogTrigger,
  DialogClose,
  DialogContent,
  DialogHeader,
  DialogFooter,
  DialogTitle,
  DialogDescription,
} from "@/components/ui/dialog";

type ForgeDialogContentProps = React.ComponentPropsWithoutRef<typeof DialogContent>;
type ForgeDialogContentRef = React.ComponentRef<typeof DialogContent>;

const ForgeDialogContent = React.forwardRef<ForgeDialogContentRef, ForgeDialogContentProps>(function ForgeDialogContent(
  props,
  ref,
) {
  return React.createElement(DialogContent, {
    ...props,
    ref,
  });
});

ForgeDialogContent.displayName = "ForgeDialogContent";

const ForgeDialog = Dialog;
const ForgeDialogPortal = DialogPortal;
const ForgeDialogOverlay = DialogOverlay;
const ForgeDialogTrigger = DialogTrigger;
const ForgeDialogClose = DialogClose;
const ForgeDialogHeader = DialogHeader;
const ForgeDialogFooter = DialogFooter;
const ForgeDialogTitle = DialogTitle;
const ForgeDialogDescription = DialogDescription;

export {
  Dialog,
  DialogPortal,
  DialogOverlay,
  DialogTrigger,
  DialogClose,
  DialogContent,
  DialogHeader,
  DialogFooter,
  DialogTitle,
  DialogDescription,
  ForgeDialog,
  ForgeDialogPortal,
  ForgeDialogOverlay,
  ForgeDialogTrigger,
  ForgeDialogClose,
  ForgeDialogContent,
  ForgeDialogHeader,
  ForgeDialogFooter,
  ForgeDialogTitle,
  ForgeDialogDescription,
};
