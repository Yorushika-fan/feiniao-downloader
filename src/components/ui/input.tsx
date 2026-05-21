import { forwardRef } from "react";
import { cn } from "@/lib/utils";

export const Input = forwardRef<
  HTMLInputElement,
  React.InputHTMLAttributes<HTMLInputElement>
>(({ className, type = "text", ...props }, ref) => (
  <input
    type={type}
    ref={ref}
    className={cn(
      "flex h-10 w-full rounded-xl border border-[hsl(var(--input))] bg-[hsl(var(--card))]",
      "px-3.5 py-2 text-[13px] outline-none",
      "placeholder:text-[hsl(var(--muted-foreground)/0.55)]",
      "transition-all duration-200",
      "focus:ring-4 focus:ring-[hsl(var(--ring)/0.12)] focus:border-[hsl(var(--ring)/0.5)]",
      "disabled:cursor-not-allowed disabled:opacity-50",
      className,
    )}
    {...props}
  />
));
Input.displayName = "Input";
