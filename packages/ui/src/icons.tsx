// Tiny inline SVG icons so @sheet-port/ui does not depend on an icon library.
type IconProps = {
  className?: string;
};

function baseProps(className: string | undefined) {
  return {
    className,
    viewBox: "0 0 16 16",
    fill: "none",
    stroke: "currentColor",
    strokeWidth: 1.75,
    strokeLinecap: "round",
    strokeLinejoin: "round",
    "aria-hidden": true
  } as const;
}

export function CheckIcon({ className }: IconProps) {
  return (
    <svg {...baseProps(className)}>
      <path d="M3 8.5 6.5 12 13 4.5" />
    </svg>
  );
}

export function XIcon({ className }: IconProps) {
  return (
    <svg {...baseProps(className)}>
      <path d="M4 4l8 8M12 4l-8 8" />
    </svg>
  );
}

export function ChevronDownIcon({ className }: IconProps) {
  return (
    <svg {...baseProps(className)}>
      <path d="M4 6l4 4 4-4" />
    </svg>
  );
}

export function ChevronUpIcon({ className }: IconProps) {
  return (
    <svg {...baseProps(className)}>
      <path d="M4 10l4-4 4 4" />
    </svg>
  );
}
