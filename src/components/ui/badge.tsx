import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

const badgeVariants = cva(
  "inline-flex items-center rounded-full px-2 py-0.5 text-[10.5px] font-semibold tracking-tight uppercase select-none transition-colors",
  {
    variants: {
      variant: {
        default:
          "bg-[hsl(var(--accent-amber)/0.12)] text-[hsl(var(--accent-amber))]",
        secondary:
          "bg-[hsl(var(--secondary))] text-[hsl(var(--secondary-foreground))]",
        outline:
          "border border-[hsl(var(--border))] text-[hsl(var(--foreground)/0.65)]",
        success:
          "bg-[hsl(var(--success)/0.13)] text-[hsl(var(--success))]",
        warning:
          "bg-[hsl(var(--warning)/0.13)] text-[hsl(var(--warning))]",
        danger:
          "bg-[hsl(var(--danger)/0.13)] text-[hsl(var(--danger))]",
      },
    },
    defaultVariants: { variant: "default" },
  },
);

export const Badge = ({
  className,
  variant,
  ...props
}: React.HTMLAttributes<HTMLDivElement> &
  VariantProps<typeof badgeVariants>) => (
  <div className={cn(badgeVariants({ variant }), className)} {...props} />
);
