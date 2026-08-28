/**
 * Apple iOS / macOS Unified Motion System Tokens & Physics Utilities
 * AeroFS Design System 2.0
 */

export const MOTION_DURATIONS = {
  fast: 160,
  standard: 240,
  emphasized: 360,
  sheet: 420,
} as const;

export const MOTION_EASINGS = {
  // Apple standard fluid spring curve - rapid deceleration with zero overshoot
  spring: 'cubic-bezier(0.32, 0.72, 0, 1)',
  // Apple fast settling curve for entrance animations
  apple: 'cubic-bezier(0.16, 1, 0.3, 1)',
  // Apple exit / dismissal curve
  appleIn: 'cubic-bezier(0.32, 0, 0.67, 0)',
  // Subtle playful spring pop for badges and alerts
  bounce: 'cubic-bezier(0.34, 1.56, 0.64, 1)',
} as const;

export type NavigationDirection = 'forward' | 'back' | 'replace';

/**
 * Computes dynamic spring settle duration based on release velocity (px/ms).
 * High-velocity flicks settle faster with momentum (180ms - 260ms),
 * while gentle drags settle smoothly (320ms).
 */
export function getDynamicSettleDuration(
  velocityX: number,
  baseDurationMs: number = 320,
  minDurationMs: number = 180
): number {
  const speed = Math.abs(velocityX); // px per ms
  if (speed < 0.1) return baseDurationMs;
  const reduction = Math.min(baseDurationMs - minDurationMs, speed * 150);
  return Math.round(Math.max(minDurationMs, baseDurationMs - reduction));
}

/**
 * Returns corresponding Vue transition name for directional folder navigation.
 */
export function getNavTransitionName(direction: NavigationDirection): string {
  switch (direction) {
    case 'forward':
      return 'nav-forward';
    case 'back':
      return 'nav-back';
    default:
      return 'nav-replace';
  }
}
