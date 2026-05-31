import * as React from "react";
import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { cn } from "@/lib/utils";

type ForgeIconButtonProps = ButtonPrimitive.Props;

const ForgeIconButton = React.forwardRef<React.ElementRef<typeof ButtonPrimitive>, ForgeIconButtonProps>(function ForgeIconButton({
  className,
  type = "button",
  ...props
}, ref) {
  return (
    <ButtonPrimitive
      ref={ref}
      type={type}
      className={cn("forge-icon-button", className)}
      {...props}
    />
  );
});

ForgeIconButton.displayName = "ForgeIconButton";

export { ForgeIconButton };
