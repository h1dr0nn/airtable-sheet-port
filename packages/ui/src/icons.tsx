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

export function XIcon({ className }: IconProps) {
  return (
    <svg {...baseProps(className)}>
      <path d="M4 4l8 8M12 4l-8 8" />
    </svg>
  );
}

export function SearchIcon({ className }: IconProps) {
  return (
    <svg {...baseProps(className)}>
      <circle cx="7" cy="7" r="4.75" />
      <path d="M10.5 10.5 14 14" />
    </svg>
  );
}

export function CheckIcon({ className }: IconProps) {
  return (
    <svg {...baseProps(className)}>
      <path d="M3 8.5l3.5 3.5L13 4.5" />
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

export function ChevronRightIcon({ className }: IconProps) {
  return (
    <svg {...baseProps(className)}>
      <path d="M6 4l4 4-4 4" />
    </svg>
  );
}

export function InfoIcon({ className }: IconProps) {
  return (
    <svg {...baseProps(className)}>
      <circle cx="8" cy="8" r="6.25" />
      <path d="M8 7.5v3.5" />
      <path d="M8 5.1h.01" />
    </svg>
  );
}

export function AlertTriangleIcon({ className }: IconProps) {
  return (
    <svg {...baseProps(className)}>
      <path d="M7.14 2.7 1.7 12.05a1 1 0 0 0 .86 1.5h10.88a1 1 0 0 0 .86-1.5L8.86 2.7a1 1 0 0 0-1.72 0Z" />
      <path d="M8 6.5v3" />
      <path d="M8 11.5h.01" />
    </svg>
  );
}

/** Open arc; pair with an animate-spin class for loading states. */
export function LoaderIcon({ className }: IconProps) {
  return (
    <svg {...baseProps(className)}>
      <path d="M14 8a6 6 0 1 1-6-6" />
    </svg>
  );
}

export function CopyIcon({ className }: IconProps) {
  return (
    <svg {...baseProps(className)}>
      <rect x="5.5" y="5.5" width="9" height="9" rx="1.5" />
      <path d="M2.75 10.5c-.7 0-1.25-.55-1.25-1.25v-6.5c0-.7.55-1.25 1.25-1.25h6.5c.7 0 1.25.55 1.25 1.25" />
    </svg>
  );
}
