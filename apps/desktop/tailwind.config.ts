import type { Config } from "tailwindcss";

// Design tokens for the "tactical telemetry / CRT terminal" direction.
// Mirrored as CSS variables in src/styles.css for non-Tailwind usage.
// Hard rules: no border-radius, no box-shadows, no gradients, no translucent panels.
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}", "../../packages/ui/src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        bg: "#0A0A0A", // deactivated CRT substrate
        surface: "#101010", // panel fill
        raised: "#141414", // raised panel fill
        ink: {
          DEFAULT: "#EAEAEA", // white phosphor
          muted: "#8A8A85" // aged phosphor
        },
        hazard: "#FF2A2A", // the ONLY accent: destructive / alert / active marker
        signal: "#4AF626", // terminal green, reserved for MCP RUNNING readout
        edge: {
          DEFAULT: "#262626", // hairlines and structural 1px borders
          strong: "#3A3A3A" // visible decoration (crosshairs, hover borders)
        }
      },
      borderColor: {
        DEFAULT: "#262626"
      },
      fontFamily: {
        // Mono is the default UI font for the whole app.
        sans: ["JetBrains Mono", "Cascadia Mono", "Consolas", "monospace"],
        mono: ["JetBrains Mono", "Cascadia Mono", "Consolas", "monospace"],
        display: ["Archivo Black", "Segoe UI Black", "Arial Black", "sans-serif"]
      },
      keyframes: {
        "fade-in": {
          from: { opacity: "0" },
          to: { opacity: "1" }
        },
        blink: {
          "0%, 49%": { opacity: "1" },
          "50%, 100%": { opacity: "0" }
        },
        "dot-pulse": {
          "0%, 100%": { opacity: "1" },
          "50%": { opacity: "0.35" }
        }
      },
      animation: {
        "fade-in": "fade-in 120ms linear",
        blink: "blink 1.2s step-end infinite",
        "dot-pulse": "dot-pulse 1.6s ease-in-out infinite"
      }
    }
  },
  plugins: []
} satisfies Config;
