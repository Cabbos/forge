import * as React from "react";
import { cn } from "@/lib/utils";

type ForgeSurfaceElement = "article" | "div" | "section";

interface ForgeSurfaceProps extends React.HTMLAttributes<HTMLElement> {
  as?: ForgeSurfaceElement;
  quiet?: boolean;
}

const ForgeSurface = React.forwardRef<HTMLElement, ForgeSurfaceProps>(function ForgeSurface({
  as: Component = "div",
  className,
  quiet = false,
  ...props
}, ref) {
  return (
    <Component
      ref={ref as React.Ref<HTMLDivElement & HTMLElement>}
      className={cn(quiet ? "forge-surface-quiet" : "forge-surface", className)}
      {...props}
    />
  );
});

ForgeSurface.displayName = "ForgeSurface";

export { ForgeSurface };
