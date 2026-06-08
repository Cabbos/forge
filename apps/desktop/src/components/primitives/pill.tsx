import * as React from "react";
import { cn } from "@/lib/utils";

type ForgePillProps = React.HTMLAttributes<HTMLSpanElement>;

const ForgePill = React.forwardRef<HTMLSpanElement, ForgePillProps>(function ForgePill({
  className,
  ...props
}, ref) {
  return (
    <span
      ref={ref}
      className={cn("forge-pill", className)}
      {...props}
    />
  );
});

ForgePill.displayName = "ForgePill";

export { ForgePill };
