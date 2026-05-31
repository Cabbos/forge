import * as React from "react";
import { Button, buttonVariants } from "@/components/ui/button";

type ForgeButtonProps = React.ComponentPropsWithoutRef<typeof Button>;
type ForgeButtonRef = React.ComponentRef<typeof Button>;

const ForgeButton = React.forwardRef<ForgeButtonRef, ForgeButtonProps>(function ForgeButton(props, ref) {
  return React.createElement(Button, {
    ...props,
    ref,
  });
});

ForgeButton.displayName = "ForgeButton";

export { Button, ForgeButton, buttonVariants };
