import * as React from "react";
import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { cn } from "@/lib/utils";

type ForgeActionButtonProps = ButtonPrimitive.Props;

const ForgeActionButton = React.forwardRef<React.ElementRef<typeof ButtonPrimitive>, ForgeActionButtonProps>(function ForgeActionButton({
  className,
  type = "button",
  ...props
}, ref) {
  return (
    <ButtonPrimitive
      ref={ref}
      type={type}
      className={cn("forge-action", className)}
      {...props}
    />
  );
});

ForgeActionButton.displayName = "ForgeActionButton";

export { ForgeActionButton };
