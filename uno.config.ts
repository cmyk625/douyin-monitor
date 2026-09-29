import {
  defineConfig,
  presetUno,
  transformerDirectives,
  transformerVariantGroup,
} from "unocss";

export default defineConfig({
  presets: [
    presetUno(),
  ],
  transformers: [
    transformerDirectives(),
    transformerVariantGroup(),
  ],
  content: {
    pipeline: {
      include: [
        /\.(vue|svelte|[jt]sx?|mdx?|astro|elm|php|phtml|html)($|\?)/,
        "src/**/*.{js,ts,jsx,tsx,vue,html}",
      ],
    },
  },
  theme: {
    colors: {
      background: "#090d16",
      foreground: "#f1f5f9",
      card: {
        DEFAULT: "#111827",
        foreground: "#f1f5f9",
        muted: "#0f172a",
      },
      panel: "#131b2a",
      border: "#1e293b",
      borderSoft: "#162032",
      input: "#334155",
      ring: "#3b82f6",
      secondary: {
        DEFAULT: "#1e293b",
        foreground: "#f1f5f9",
      },
      popover: {
        DEFAULT: "#111827",
        foreground: "#f1f5f9",
      },
      primary: {
        DEFAULT: "#3b82f6",
        hover: "#2563eb",
        foreground: "#ffffff",
      },
      accent: {
        DEFAULT: "#10b981",
        hover: "#059669",
        foreground: "#ffffff",
      },
      destructive: {
        DEFAULT: "#ef4444",
        foreground: "#ffffff",
      },
      warning: {
        DEFAULT: "#f59e0b",
        foreground: "#ffffff",
      },
      muted: {
        DEFAULT: "#1e293b",
        foreground: "#94a3b8",
      },
    },
  },
  shortcuts: {
    "btn-base": "inline-flex items-center justify-center gap-1.5 whitespace-nowrap rounded-lg text-xs font-medium transition-all focus:outline-none select-none cursor-pointer disabled:pointer-events-none disabled:opacity-40",
    "btn-default": "btn-base bg-blue-600 text-white shadow-sm hover:bg-blue-500 active:scale-[0.98]",
    "btn-secondary": "btn-base bg-slate-800 text-slate-100 border border-slate-700/60 hover:bg-slate-700/80 active:scale-[0.98]",
    "btn-outline": "btn-base border border-slate-700/80 bg-transparent text-slate-200 hover:bg-slate-800/80 active:scale-[0.98]",
    "btn-ghost": "btn-base text-slate-300 hover:bg-slate-800/60 hover:text-white active:scale-[0.98]",
    "btn-destructive": "btn-base bg-red-500/15 text-red-400 border border-red-500/30 hover:bg-red-500/25 active:scale-[0.98]",
    "card-base": "rounded-xl border border-slate-800 bg-slate-900/80 backdrop-blur-md shadow-sm",
  },
});
