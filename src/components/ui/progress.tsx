import { forwardRef } from "react";
import { cn } from "@/lib/utils";

interface ProgressProps extends React.HTMLAttributes<HTMLDivElement> {
  value?: number;
  indeterminate?: boolean;
  variant?: "default" | "success" | "danger";
}

export const Progress = forwardRef<HTMLDivElement, ProgressProps>(
  (
    { value = 0, indeterminate, variant = "default", className, ...props },
    ref,
  ) => {
    const fillClass =
      variant === "success"
        ? "bg-gradient-to-r from-[hsl(var(--success))] to-emerald-400"
        : variant === "danger"
          ? "bg-gradient-to-r from-[hsl(var(--danger))] to-rose-400"
          : "bg-gradient-amber";

    return (
      <div
        ref={ref}
        className={cn(
          "relative h-1.5 w-full overflow-hidden rounded-full bg-[hsl(var(--muted))]",
          className,
        )}
        {...props}
      >
        {indeterminate ? (
          <div className={cn("indeterminate-bar rounded-full", fillClass)} />
        ) : (
          <div
            className={cn(
              "relative h-full rounded-full progress-shine transition-[width] duration-500 ease-out",
              fillClass,
            )}
            style={{ width: `${Math.min(100, Math.max(0, value))}%` }}
          />
        )}
      </div>
    );
  },
);
Progress.displayName = "Progress";
