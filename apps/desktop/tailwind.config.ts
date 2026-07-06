import type { Config } from "tailwindcss";

// Semantic design tokens for the dual-mode "precision instrument" theme.
// Every color resolves to an RGB-channel CSS variable defined in
// src/styles.css (:root = light, .dark = dark), so components never
// hardcode a palette and opacity modifiers keep working.
function token(variable: string): string {
  return `rgb(var(${variable}) / <alpha-value>)`;
}

export default {
  darkMode: ["class"],
  content: ["./index.html", "./src/**/*.{ts,tsx}", "../../packages/ui/src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        bg: token("--bg"),
        surface: token("--surface"),
        raised: token("--raised"),
        overlay: token("--overlay"),
        ink: {
          DEFAULT: token("--ink"),
          muted: token("--ink-muted"),
          faint: token("--ink-faint")
        },
        edge: {
          DEFAULT: token("--edge"),
          strong: token("--edge-strong")
        },
        accent: {
          DEFAULT: token("--accent"),
          hover: token("--accent-hover"),
          ink: token("--accent-ink")
        },
        success: token("--success"),
        warning: token("--warning"),
        danger: {
          DEFAULT: token("--danger"),
          solid: token("--danger-solid"),
          "solid-hover": token("--danger-solid-hover")
        }
      },
      borderColor: {
        DEFAULT: "rgb(var(--edge) / 1)"
      },
      borderRadius: {
        card: "10px"
      },
      boxShadow: {
        card: "var(--shadow-card)",
        pop: "var(--shadow-pop)"
      },
      fontFamily: {
        sans: ["Inter Variable", "Inter", "-apple-system", "Segoe UI", "sans-serif"],
        mono: ["JetBrains Mono", "Cascadia Mono", "Consolas", "monospace"]
      },
      keyframes: {
        "fade-in": {
          from: { opacity: "0" },
          to: { opacity: "1" }
        },
        "scale-in": {
          from: { opacity: "0", transform: "scale(0.98)" },
          to: { opacity: "1", transform: "scale(1)" }
        },
        "dot-pulse": {
          "0%, 100%": { opacity: "1" },
          "50%": { opacity: "0.4" }
        }
      },
      animation: {
        "fade-in": "fade-in 140ms cubic-bezier(0.4, 0, 0.2, 1)",
        "fade-up": "fade-up 200ms cubic-bezier(0.4, 0, 0.2, 1)",
        "scale-in": "scale-in 160ms cubic-bezier(0.4, 0, 0.2, 1)",
        "dot-pulse": "dot-pulse 2s ease-in-out infinite"
      }
    }
  },
  plugins: []
} satisfies Config;
