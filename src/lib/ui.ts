import { tv } from "tailwind-variants";

export const button = tv({
  base: "rounded-lg px-4 py-2 text-sm font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-50",
  variants: {
    intent: {
      primary: "bg-blue-600 text-white hover:bg-blue-500",
      secondary: "bg-zinc-700 text-zinc-100 hover:bg-zinc-600",
      danger: "bg-red-600/20 text-red-400 hover:bg-red-600/30",
    },
  },
  defaultVariants: {
    intent: "primary",
  },
});

export const input = tv({
  base: "w-full rounded-lg border border-zinc-700 bg-zinc-800 px-3 py-2 text-sm text-zinc-100 placeholder-zinc-500 outline-none focus:border-blue-500",
});

export const card = tv({
  base: "rounded-xl border border-zinc-800 bg-zinc-900 p-4",
});

export const tab = tv({
  base: "rounded-lg px-4 py-2 text-sm font-medium transition-colors",
  variants: {
    active: {
      true: "bg-zinc-800 text-zinc-100",
      false: "text-zinc-400 hover:text-zinc-200",
    },
  },
});
