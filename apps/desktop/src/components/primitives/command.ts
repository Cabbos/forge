import * as React from "react";
import {
  Command,
  CommandDialog,
  CommandInput,
  CommandList,
  CommandEmpty,
  CommandGroup,
  CommandItem,
  CommandShortcut,
  CommandSeparator,
} from "@/components/ui/command";

type ForgeCommandRef = React.ComponentRef<typeof Command>;
type ForgeCommandProps = React.ComponentPropsWithoutRef<typeof Command>;
type ForgeCommandInputRef = React.ComponentRef<typeof CommandInput>;
type ForgeCommandInputProps = React.ComponentPropsWithoutRef<typeof CommandInput>;
type ForgeCommandListRef = React.ComponentRef<typeof CommandList>;
type ForgeCommandListProps = React.ComponentPropsWithoutRef<typeof CommandList>;
type ForgeCommandEmptyRef = React.ComponentRef<typeof CommandEmpty>;
type ForgeCommandEmptyProps = React.ComponentPropsWithoutRef<typeof CommandEmpty>;
type ForgeCommandGroupRef = React.ComponentRef<typeof CommandGroup>;
type ForgeCommandGroupProps = React.ComponentPropsWithoutRef<typeof CommandGroup>;
type ForgeCommandItemRef = React.ComponentRef<typeof CommandItem>;
type ForgeCommandItemProps = React.ComponentPropsWithoutRef<typeof CommandItem>;
type ForgeCommandShortcutRef = React.ComponentRef<typeof CommandShortcut>;
type ForgeCommandShortcutProps = React.ComponentPropsWithoutRef<typeof CommandShortcut>;
type ForgeCommandSeparatorRef = React.ComponentRef<typeof CommandSeparator>;
type ForgeCommandSeparatorProps = React.ComponentPropsWithoutRef<typeof CommandSeparator>;

const ForgeCommand = React.forwardRef<ForgeCommandRef, ForgeCommandProps>(function ForgeCommand(props, ref) {
  return React.createElement(Command, { ...props, ref });
});

const ForgeCommandInput = React.forwardRef<ForgeCommandInputRef, ForgeCommandInputProps>(function ForgeCommandInput(props, ref) {
  return React.createElement(CommandInput, { ...props, ref });
});

const ForgeCommandList = React.forwardRef<ForgeCommandListRef, ForgeCommandListProps>(function ForgeCommandList(props, ref) {
  return React.createElement(CommandList, { ...props, ref });
});

const ForgeCommandEmpty = React.forwardRef<ForgeCommandEmptyRef, ForgeCommandEmptyProps>(function ForgeCommandEmpty(props, ref) {
  return React.createElement(CommandEmpty, { ...props, ref });
});

const ForgeCommandGroup = React.forwardRef<ForgeCommandGroupRef, ForgeCommandGroupProps>(function ForgeCommandGroup(props, ref) {
  return React.createElement(CommandGroup, { ...props, ref });
});

const ForgeCommandItem = React.forwardRef<ForgeCommandItemRef, ForgeCommandItemProps>(function ForgeCommandItem(props, ref) {
  return React.createElement(CommandItem, { ...props, ref });
});

const ForgeCommandShortcut = React.forwardRef<ForgeCommandShortcutRef, ForgeCommandShortcutProps>(function ForgeCommandShortcut(
  props,
  ref,
) {
  return React.createElement(CommandShortcut, { ...props, ref });
});

const ForgeCommandSeparator = React.forwardRef<ForgeCommandSeparatorRef, ForgeCommandSeparatorProps>(function ForgeCommandSeparator(
  props,
  ref,
) {
  return React.createElement(CommandSeparator, { ...props, ref });
});

const ForgeCommandDialog = CommandDialog;

ForgeCommand.displayName = "ForgeCommand";
ForgeCommandInput.displayName = "ForgeCommandInput";
ForgeCommandList.displayName = "ForgeCommandList";
ForgeCommandEmpty.displayName = "ForgeCommandEmpty";
ForgeCommandGroup.displayName = "ForgeCommandGroup";
ForgeCommandItem.displayName = "ForgeCommandItem";
ForgeCommandShortcut.displayName = "ForgeCommandShortcut";
ForgeCommandSeparator.displayName = "ForgeCommandSeparator";

export {
  Command,
  CommandDialog,
  CommandInput,
  CommandList,
  CommandEmpty,
  CommandGroup,
  CommandItem,
  CommandShortcut,
  CommandSeparator,
  ForgeCommand,
  ForgeCommandDialog,
  ForgeCommandInput,
  ForgeCommandList,
  ForgeCommandEmpty,
  ForgeCommandGroup,
  ForgeCommandItem,
  ForgeCommandShortcut,
  ForgeCommandSeparator,
};
