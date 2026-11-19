# 0002 — Apple-inspired glass design language

Date: 2026-11-19 · Status: accepted (owner request) · Supersedes: Plan tab "Brand guidelines"
sections on colour, typography, shape and motion; CLAUDE.md UI rules v1.

## Context
The owner wants Unison to feel like Apple's current design language (Liquid Glass, introduced
WWDC 2025): inspiration, not imitation. Unison ships on Windows, so Apple's own assets can't be
used: SF Pro / SF Symbols are licensed only for Apple platforms, and Apple Music's look (pink-red
accent, layouts, icons) must not be copied.

## Decision
1. **Two layers.** Content (lists, art, text) is opaque and calm. The navigation/control layer
   (title bar, sidebar, player bar, menus, popovers, sheets, toasts) is glass and floats above
   content. Never glass-on-glass; never glass behind long text or table rows.
2. **Materials.** Window backdrop: Windows 11 **Mica** by default, **Acrylic** or **Off** as a
   user setting (T-71), plus a glass-intensity setting. Windows 10 falls back to opaque. In-app
   glass = `backdrop-filter` blur + saturation + tint + 1 px specular rim; an optional SVG
   refraction filter only on small hero controls (WebView2 is Chromium, so it works; it is a
   progressive enhancement). `prefers-reduced-transparency` and the "Off" setting make every glass
   surface opaque.
3. **Colour.** Apple-style label tiers (primary / secondary / tertiary / quaternary as opacities)
   and fill tiers on near-neutral greys. **Accent adapts to the current artwork** (computed in Rust
   from the cover, contrast-corrected to ≥ 3:1 for controls, ≥ 4.5:1 for text) with violet
   `#8B70FF` / `#6A4DF5` as the fallback. Platform colours stay inside source badges only.
   Gradients allowed for: album-art ambient backdrop (Now Playing, page headers), glass rims,
   scrims, skeleton shimmer.
4. **Type.** One family: **Inter** (variable, with optical sizing so large titles get the display
   cut) using Apple-like dynamic tracking. Numbers use tabular figures, not a monospace font.
   Space Grotesk and JetBrains Mono are dropped.
5. **Shape.** Capsule buttons, segmented controls and search fields; concentric corners (inner
   radius = outer radius − padding); 12 px cards, 16–20 px floating panels and sheets.
6. **Motion.** Physical springs (Apple's "smooth" = no overshoot for most motion, "snappy" =
   slight overshoot for press and popover/sheet entry), implemented with CSS `linear()` easings
   generated from spring parameters. Hover stays a tone change; press scales to 0.97. Reduced
   motion → crossfades only.
7. **Icons.** lucide-react (open licence), 1.5 px stroke, label colours. No SF Symbols.
8. **Lyrics.** Time-synced lyrics in Now Playing via LRCLIB (free, no key), new task T-45 after
   playback. Fetched in Rust like every other network call.

## Consequences
- CLAUDE.md UI rules and spec §2/§3/§6/§11 updated; new task T-03a redoes the T-03 kit.
- Glass costs GPU time: it is limited to the chrome layer; performance budgets (§10) still apply.
- LRCLIB lyrics are community-sourced; lyric licensing must be reviewed before public launch
  (added to §12 checklist).

## Addendum (2026-11-19) — Finder-style sidebar
Owner request: the sidebar follows macOS Finder. Section headers are bold, muted and collapsible
(chevron on hover); rows are 28 px with accent-tinted icons and primary-colour labels; the
selected row is a grey rounded fill instead of the 2 px accent bar; no wordmark at the top (empty
drag strip). Accent-tinted icons are allowed only in the sidebar.
