import { type VariantProps, cva } from "class-variance-authority";

export { default as Badge } from "./Badge.vue";

export const badgeVariants = cva(
  "inline-flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-[11px] font-medium transition-colors select-none whitespace-nowrap shrink-0 leading-none h-fit max-w-full",
  {
    variants: {
      variant: {
        default:
          "border-transparent bg-blue-600 text-white shadow-sm hover:bg-blue-500",
        secondary:
          "border-slate-700/80 bg-slate-800/80 text-slate-300",
        destructive:
          "border-red-500/30 bg-red-500/15 text-red-400",
        outline: "text-slate-300 border-slate-700",
        success:
          "border-emerald-500/30 bg-emerald-500/15 text-emerald-400",
        warning:
          "border-amber-500/30 bg-amber-500/15 text-amber-400",
        info:
          "border-sky-500/30 bg-sky-500/15 text-sky-400",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  },
);

export type BadgeVariants = VariantProps<typeof badgeVariants>;
