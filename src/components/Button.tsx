import type { ButtonHTMLAttributes } from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

const button = cva(
  "inline-flex items-center justify-center gap-2 rounded-control font-medium whitespace-nowrap transition-colors disabled:pointer-events-none disabled:opacity-45",
  {
    variants: {
      variant: {
        primary: "bg-signal text-signal-ink hover:bg-[#ffc555]",
        secondary: "border border-rivet bg-bay text-alu hover:border-alu-dim/60",
        ghost: "text-alu-dim hover:bg-rivet/40 hover:text-alu",
        danger: "border border-flare/50 text-flare hover:bg-flare hover:text-white",
      },
      size: {
        sm: "h-8 px-3 text-caption",
        md: "h-9 px-4",
        lg: "h-12 px-7 font-display text-[1.375rem] font-bold tracking-[0.02em]",
        icon: "size-9",
      },
    },
    defaultVariants: { variant: "secondary", size: "md" },
  },
);

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & VariantProps<typeof button>;

export function Button({ className, variant, size, type = "button", ...props }: ButtonProps) {
  return <button type={type} className={cn(button({ variant, size }), className)} {...props} />;
}
