import * as React from "react";
import { Input } from "@/components/ui/input";

type ForgeTextInputProps = React.ComponentPropsWithoutRef<typeof Input>;

const ForgeTextInput = React.forwardRef<HTMLInputElement, ForgeTextInputProps>(function ForgeTextInput(props, ref) {
  return React.createElement(Input, {
    ...props,
    ref,
  });
});

ForgeTextInput.displayName = "ForgeTextInput";

export { Input, ForgeTextInput };
