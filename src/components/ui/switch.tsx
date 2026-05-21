import * as SwitchPrimitive from "@radix-ui/react-switch";
import { forwardRef } from "react";
import { cn } from "@/lib/utils";

export const Switch = forwardRef<
  React.ElementRef<typeof SwitchPrimitive.Root>,
  React.ComponentPropsWithoutRef<typeof SwitchPrimitive.Root>
>(({ className, ...props }, ref) => (
  <SwitchPrimitive.Root
    ref={ref}
    className={cn(
      "peer inline-flex h-[24px] w-[42px] shrink-0 cursor-pointer items-center rounded-full",
      "border-2 border-transparent",
      "transition-all duration-200 ease-out",
      "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[hsl(var(--ring)/0.4)] focus-visible:ring-offset-1",
      "disabled:cursor-not-allowed disabled:opacity-50",
      "data-[state=checked]:bg-gradient-amber data-[state=checked]:shadow-[0_2px_6px_rgba(202,138,4,0.3)]",
      "data-[state=unchecked]:bg-[hsl(var(--muted))]",
      className,
    )}
    {...props}
  >
    <SwitchPrimitive.Thumb
      className={cn(
        "pointer-events-none block h-[18px] w-[18px] rounded-full bg-white",
        "shadow-[0_1px_3px_rgba(0,0,0,0.2),0_1px_2px_rgba(0,0,0,0.15)]",
        "ring-0 transition-transform duration-200 ease-out",
        "data-[state=checked]:translate-x-[18px]",
        "data-[state=unchecked]:translate-x-[1px]",
      )}
    />
  </SwitchPrimitive.Root>
));
Switch.displayName = "Switch";
