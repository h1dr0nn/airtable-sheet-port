import { cn, Tooltip, TooltipContent, TooltipTrigger } from "@sheet-port/ui";
import { formatAbsoluteTime, formatRelativeTime } from "../lib/format.js";

type RelativeTimeProps = {
  iso: string;
  className?: string;
};

/** Relative timestamp with the absolute time in a tooltip. */
export function RelativeTime({ iso, className }: RelativeTimeProps) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <time dateTime={iso} className={cn("cursor-default whitespace-nowrap", className)}>
          {formatRelativeTime(iso)}
        </time>
      </TooltipTrigger>
      <TooltipContent className="font-mono">{formatAbsoluteTime(iso)}</TooltipContent>
    </Tooltip>
  );
}
