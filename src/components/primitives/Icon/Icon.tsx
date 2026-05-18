import type { SVGAttributes } from 'react';
import { FILLED_ICONS, ICONS, type IconName } from './icons';

export interface IconProps extends Omit<SVGAttributes<SVGSVGElement>, 'fill' | 'stroke'> {
  name: IconName;
  size?: number;
  stroke?: string;
  fill?: string;
  strokeWidth?: number;
}

export function Icon({
  name,
  size = 18,
  stroke = 'currentColor',
  fill = 'none',
  strokeWidth = 1.7,
  className,
  ...rest
}: IconProps) {
  const d = ICONS[name];
  const filled = FILLED_ICONS.has(name);
  return (
    <svg
      aria-hidden="true"
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill={filled ? stroke : fill}
      stroke={filled ? 'none' : stroke}
      strokeWidth={strokeWidth}
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className}
      {...rest}
    >
      <path d={d} />
    </svg>
  );
}

export type { IconName };
