import { AnimatePresence, motion, useReducedMotion } from "framer-motion";
import type { ReactNode } from "react";

// Shares the app's --dur-normal / --ease-standard vocabulary (see styles.css),
// expressed as framer-motion values so the JS-driven crossfade matches the
// CSS-driven transitions elsewhere.
const SCREEN_DURATION = 0.2;
const SCREEN_EASE = [0.4, 0, 0.2, 1] as const;
const SCREEN_SHIFT_Y = 6;

type AnimatedScreenProps = {
  /** Stable id of the active screen; changing it triggers the crossfade. */
  screenKey: string;
  children: ReactNode;
};

/** Fades and lifts the active screen when the key changes. Reduced-motion users
 * get an instant swap (no fade, no shift). Kept in @sheet-port/ui because
 * framer-motion is a dependency here, not in the app package. */
export function AnimatedScreen({ screenKey, children }: AnimatedScreenProps) {
  const reduceMotion = useReducedMotion();

  // Reduced motion: render the active screen directly with no crossfade so it
  // can never be left mid-animation at a low opacity. Keyed so React still
  // remounts on screen change.
  if (reduceMotion) {
    return <div key={screenKey}>{children}</div>;
  }

  return (
    <AnimatePresence mode="wait" initial={false}>
      <motion.div
        key={screenKey}
        initial={{ opacity: 0, y: SCREEN_SHIFT_Y }}
        animate={{ opacity: 1, y: 0 }}
        exit={{ opacity: 0, y: -SCREEN_SHIFT_Y }}
        transition={{ duration: SCREEN_DURATION, ease: SCREEN_EASE }}
      >
        {children}
      </motion.div>
    </AnimatePresence>
  );
}
