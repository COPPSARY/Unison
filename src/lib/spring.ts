/**
 * Spring easings (ADR 0002). Converts Apple-style spring parameters (perceptual duration +
 * bounce) into a CSS `linear()` easing plus the time the spring needs to settle.
 */
export type SpringParams = {
  /** Perceptual duration in seconds (period of the undamped spring). */
  duration: number;
  /** 0 = no overshoot (critically damped); 0.15 = slight overshoot. */
  bounce: number;
};

export const SPRINGS = {
  /** Default for most motion: no overshoot. */
  smooth: { duration: 0.45, bounce: 0 },
  /** Press feedback, popovers, sheets: a hint of overshoot. */
  snappy: { duration: 0.32, bounce: 0.15 },
} as const satisfies Record<string, SpringParams>;

export type SpringName = keyof typeof SPRINGS;

/** Unit step response x(t) of a spring from 0 to 1 with mass 1. */
export function springPosition({ duration, bounce }: SpringParams, t: number): number {
  const w0 = (2 * Math.PI) / duration;
  const zeta = 1 - bounce;
  if (zeta >= 1) return 1 - Math.exp(-w0 * t) * (1 + w0 * t);
  const wd = w0 * Math.sqrt(1 - zeta * zeta);
  const decay = Math.exp(-zeta * w0 * t);
  return 1 - decay * (Math.cos(wd * t) + ((zeta * w0) / wd) * Math.sin(wd * t));
}

/** Time in seconds after which the spring stays within `epsilon` of its target. */
export function settleTime(params: SpringParams, epsilon = 0.001): number {
  const step = 1 / 1000;
  let lastOutside = 0;
  for (let i = 0; i <= 5000; i++) {
    const t = i * step;
    if (Math.abs(springPosition(params, t) - 1) > epsilon) lastOutside = t;
  }
  return lastOutside + step;
}

export type SpringEasing = { easing: string; durationMs: number };

/** CSS `linear()` easing sampled from the spring, with its settle duration. */
export function springEasing(params: SpringParams, samples = 48): SpringEasing {
  const total = settleTime(params);
  const points: string[] = [];
  for (let i = 0; i <= samples; i++) {
    const value = i === samples ? 1 : springPosition(params, (total * i) / samples);
    points.push(String(Math.round(value * 10000) / 10000));
  }
  return { easing: `linear(${points.join(", ")})`, durationMs: Math.round(total * 1000) };
}

/** Publishes every spring as `--spring-<name>` and `--spring-<name>-duration` on <html>. */
export function installSprings(root: HTMLElement = document.documentElement): void {
  for (const [name, params] of Object.entries(SPRINGS)) {
    const { easing, durationMs } = springEasing(params);
    root.style.setProperty(`--spring-${name}`, easing);
    root.style.setProperty(`--spring-${name}-duration`, `${durationMs}ms`);
  }
}
