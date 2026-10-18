import { LUCIDE_ALIASES, LUCIDE_PATHS } from "./lucide.generated";

export function hasIcon(name: string): boolean {
  return (LUCIDE_ALIASES[name] ?? name) in LUCIDE_PATHS;
}

/** A Lucide line icon at the design's 1.5 px stroke, whatever its size. Decorative unless `label` is given. */
export function Icon({
  name,
  size = 16,
  label,
  className,
}: {
  name: string;
  size?: number;
  label?: string;
  className?: string;
}) {
  const path = LUCIDE_PATHS[LUCIDE_ALIASES[name] ?? name];
  return (
    <svg
      className={className ? `vc-icon ${className}` : "vc-icon"}
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.5}
      strokeLinecap="round"
      strokeLinejoin="round"
      role={label ? "img" : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : true}
      data-icon={name}
      data-missing={path ? undefined : ""}
    >
      {path ? <path d={path} vectorEffect="non-scaling-stroke" /> : null}
    </svg>
  );
}
