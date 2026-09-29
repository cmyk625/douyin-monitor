import { type VariantProps, cva } from "class-variance-authority";

export { default as Button } from "./Button.vue";

export const buttonVariants = cva(
  "inline-flex items-center justify-center gap-1.5 whitespace-nowrap rounded-lg text-xs font-medium transition-all focus:outline-none select-none cursor-pointer disabled:pointer-events-none disabled:opacity-40",
  {
    variants: {
      variant: {
        default:
          "bg-blue-600 text-white shadow-sm hover:bg-blue-500 active:scale-[0.98]",
        destructive:
          "bg-red-500/15 text-red-400 border border-red-500/30 hover:bg-red-500/25 active:scale-[0.98]",
        outline:
          "border border-slate-700/80 bg-slate-800/40 text-slate-200 hover:bg-slate-800 hover:text-white active:scale-[0.98]",
        secondary:
          "bg-slate-800 text-slate-100 border border-slate-700/60 hover:bg-slate-700/80 active:scale-[0.98]",
        ghost:
          "text-slate-300 hover:bg-slate-800/60 hover:text-white active:scale-[0.98]",
        link: "text-blue-400 underline-offset-4 hover:underline",
        emerald:
          "bg-emerald-600 text-white shadow-sm hover:bg-emerald-500 active:scale-[0.98]",
      },
      size: {
        default: "h-8 px-3 py-1.5",
        sm: "h-7 rounded-md px-2.5 text-xs",
        lg: "h-9 rounded-md px-5 text-sm",
        icon: "h-8 w-8",
        "icon-sm": "h-7 w-7 p-0",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  },
);

export type ButtonVariants = VariantProps<typeof buttonVariants>;
