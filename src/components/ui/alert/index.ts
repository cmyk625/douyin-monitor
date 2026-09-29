import { type VariantProps, cva } from "class-variance-authority";

export { default as Alert } from "./Alert.vue";
export { default as AlertTitle } from "./AlertTitle.vue";
export { default as AlertDescription } from "./AlertDescription.vue";

export const alertVariants = cva(
  "relative w-full rounded-lg border px-4 py-3 text-xs [&>svg+div]:translate-y-[-3px] [&>svg]:absolute [&>svg]:left-4 [&>svg]:top-3.5 [&>svg]:text-foreground [&>svg~*]:pl-7",
  {
    variants: {
      variant: {
        default: "bg-background text-foreground border-border",
        destructive:
          "border-destructive/40 text-destructive dark:border-destructive/30 bg-destructive/10 [&>svg]:text-destructive",
        warning:
          "border-amber-500/40 text-amber-300 bg-amber-500/10 [&>svg]:text-amber-400",
        info:
          "border-sky-500/40 text-sky-300 bg-sky-500/10 [&>svg]:text-sky-400",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  },
);

export type AlertVariants = VariantProps<typeof alertVariants>;
