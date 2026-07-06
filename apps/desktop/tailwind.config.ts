import type { Config } from "tailwindcss";

// Design tokens for the dark "capability broker console" direction.
// Mirrored as CSS variables in src/styles.css for non-Tailwind usage.
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}", "../../packages/ui/src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        bg: "#0A0D12",
        surface: "#10141B",
        raised: "#161B24",
        ink: {
          DEFAULT: "#E6EAF2",
          muted: "#8A94A6"
        },
        accent: "#34D399",
        warning: "#FBBF24",
        danger: "#F87171",
        info: "#60A5FA",
        edge: {
          DEFAULT: "rgba(148, 163, 184, 0.08)",
          strong: "rgba(148, 163, 184, 0.18)"
        }
      },
      borderColor: {
        DEFAULT: "rgba(148, 163, 184, 0.08)"
      },
      fontFamily: {
        sans: ["Inter var", "Inter", "system-ui", "sans-serif"],
        mono: ["JetBrains Mono", "ui-monospace", "Consolas", "monospace"]
      },
      boxShadow: {
        card: "0 1px 2px rgba(0, 0, 0, 0.35), inset 0 1px 0 rgba(255, 255, 255, 0.03)",
        raised: "0 12px 32px rgba(0, 0, 0, 0.45)"
      },
      keyframes: {
        "fade-in": {
          from: { opacity: "0" },
          to: { opacity: "1" }
        },
        "scale-in": {
          from: { opacity: "0", transform: "translate(-50%, -50%) scale(0.97)" },
          to: { opacity: "1", transform: "translate(-50%, -50%) scale(1)" }
        },
        "toast-in": {
          from: { opacity: "0", transform: "translateY(8px)" },
          to: { opacity: "1", transform: "translateY(0)" }
        }
      },
      animation: {
        "fade-in": "fade-in 150ms ease-out",
        "scale-in": "scale-in 180ms cubic-bezier(0.16, 1, 0.3, 1)",
        "toast-in": "toast-in 200ms cubic-bezier(0.16, 1, 0.3, 1)"
      }
    }
  },
  plugins: []
} satisfies Config;
