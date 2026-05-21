import { forwardRef } from "react";
import { cn } from "@/lib/utils";

interface SelectProps extends React.SelectHTMLAttributes<HTMLSelectElement> {}

export const Select = forwardRef<HTMLSelectElement, SelectProps>(
  ({ className, children, ...props }, ref) => (
    <select
      ref={ref}
      className={cn(
        "h-10 w-full rounded-xl border border-[hsl(var(--input))] bg-[hsl(var(--card))]",
        "px-3.5 text-[13px] outline-none cursor-pointer",
        "transition-all duration-200",
        "focus:ring-4 focus:ring-[hsl(var(--ring)/0.12)] focus:border-[hsl(var(--ring)/0.5)]",
        "disabled:cursor-not-allowed disabled:opacity-50",
        "appearance-none bg-no-repeat pr-9",
        className,
      )}
      style={{
        backgroundImage:
          "url(\"data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='16' height='16' viewBox='0 0 24 24' fill='none' stroke='%23999' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><polyline points='6 9 12 15 18 9'/></svg>\")",
        backgroundPosition: "right 0.75rem center",
        backgroundSize: "1rem",
      }}
      {...props}
    >
      {children}
    </select>
  ),
);
Select.displayName = "Select";
