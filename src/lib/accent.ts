/**
 * Adaptive accent (ADR 0002): the accent follows the current artwork colour, corrected for
 * contrast, with violet as the fallback. Colour extraction itself happens in Rust (T-42).
 */
export type Rgb = { r: number; g: number; b: number };
export type ThemeName = "dark" | "light";

/** Contrast reference: the content background of each theme (mirrors tokens.css). */
const BACKGROUND: Record<ThemeName, Rgb> = {
  dark: { r: 18, g: 18, b: 22 },
  light: { r: 244, g: 244, b: 246 },
};

export const FALLBACK_ACCENT: Record<ThemeName, Rgb> = {
  dark: { r: 139, g: 112, b: 255 },
  light: { r: 106, g: 77, b: 245 },
};

const WHITE: Rgb = { r: 255, g: 255, b: 255 };
const INK: Rgb = { r: 12, g: 12, b: 16 };

function channel(c: number): number {
  const s = c / 255;
  return s <= 0.04045 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
}

export function luminance({ r, g, b }: Rgb): number {
  return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
}

/** WCAG contrast ratio between two colours. */
export function contrast(a: Rgb, b: Rgb): number {
  const la = luminance(a);
  const lb = luminance(b);
  return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
}

type Hsl = { h: number; s: number; l: number };

function toHsl({ r, g, b }: Rgb): Hsl {
  const rn = r / 255;
  const gn = g / 255;
  const bn = b / 255;
  const max = Math.max(rn, gn, bn);
  const min = Math.min(rn, gn, bn);
  const l = (max + min) / 2;
  const d = max - min;
  if (d === 0) return { h: 0, s: 0, l };
  const s = d / (1 - Math.abs(2 * l - 1));
  let h: number;
  if (max === rn) h = ((gn - bn) / d) % 6;
  else if (max === gn) h = (bn - rn) / d + 2;
  else h = (rn - gn) / d + 4;
  return { h: (h * 60 + 360) % 360, s, l };
}

function toRgb({ h, s, l }: Hsl): Rgb {
  const c = (1 - Math.abs(2 * l - 1)) * s;
  const x = c * (1 - Math.abs(((h / 60) % 2) - 1));
  const m = l - c / 2;
  let rgb: [number, number, number];
  if (h < 60) rgb = [c, x, 0];
  else if (h < 120) rgb = [x, c, 0];
  else if (h < 180) rgb = [0, c, x];
  else if (h < 240) rgb = [0, x, c];
  else if (h < 300) rgb = [x, 0, c];
  else rgb = [c, 0, x];
  return {
    r: Math.round((rgb[0] + m) * 255),
    g: Math.round((rgb[1] + m) * 255),
    b: Math.round((rgb[2] + m) * 255),
  };
}

/** Minimum contrast of the accent against the background (controls, WCAG 1.4.11). */
const MIN_UI_CONTRAST = 3;
/** Below this saturation the artwork is treated as greyscale and the fallback is used. */
const MIN_SATURATION = 0.18;

export type Accent = { accent: Rgb; foreground: Rgb };

/**
 * Turns an artwork colour into a usable accent for the theme: keeps the hue, keeps it vivid,
 * and moves lightness until it reaches 3:1 against the background. The foreground is white or
 * ink, whichever reads better.
 */
export function deriveAccent(source: Rgb | null, theme: ThemeName): Accent {
  const bg = BACKGROUND[theme];
  let hsl = source ? toHsl(source) : null;
  if (!hsl || hsl.s < MIN_SATURATION) hsl = toHsl(FALLBACK_ACCENT[theme]);
  hsl = { ...hsl, s: Math.min(Math.max(hsl.s, 0.55), 0.9) };

  // Start from a theme-appropriate lightness, then walk until contrast is met.
  let l = theme === "dark" ? Math.max(hsl.l, 0.55) : Math.min(hsl.l, 0.5);
  const step = theme === "dark" ? 0.01 : -0.01;
  let rgb = toRgb({ ...hsl, l });
  for (let i = 0; i < 100 && contrast(rgb, bg) < MIN_UI_CONTRAST; i++) {
    l = Math.min(Math.max(l + step, 0), 1);
    rgb = toRgb({ ...hsl, l });
  }
  const foreground = contrast(WHITE, rgb) >= contrast(INK, rgb) ? WHITE : INK;
  return { accent: rgb, foreground };
}

const css = ({ r, g, b }: Rgb) => `rgb(${r} ${g} ${b})`;

/** Writes the accent CSS variables used by tokens.css. */
export function applyAccent(
  source: Rgb | null,
  theme: ThemeName,
  root: HTMLElement = document.documentElement,
): Accent {
  const result = deriveAccent(source, theme);
  const { r, g, b } = result.accent;
  root.style.setProperty("--accent", css(result.accent));
  root.style.setProperty("--accent-foreground", css(result.foreground));
  root.style.setProperty("--accent-rgb", `${r} ${g} ${b}`);
  return result;
}
