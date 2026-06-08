import * as React from "react";
import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { cn } from "@/lib/utils";

type ForgeControlButtonProps = ButtonPrimitive.Props;

const ForgeControlButton = React.forwardRef<React.ElementRef<typeof ButtonPrimitive>, ForgeControlButtonProps>(function ForgeControlButton({
  className,
  type = "button",
  ...props
}, ref) {
  return (
    <ButtonPrimitive
      ref={ref}
      type={type}
      className={cn("forge-control-surface", className)}
      {...props}
    />
  );
});

ForgeControlButton.displayName = "ForgeControlButton";

export { ForgeControlButton };
