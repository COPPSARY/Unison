/**
 * SVG filter defs used by `.glass-lens` (backdrop refraction). Mounted once at the app root.
 * The displacement map is neutral (128,128) in the middle and ramps towards the edges, so the
 * backdrop bends near the rim like a thick glass lens. Chromium/WebView2 only; other engines
 * ignore the url() and keep the blur.
 */
const MAP = [
  "<svg xmlns='http://www.w3.org/2000/svg' width='200' height='200' viewBox='0 0 200 200'>",
  "<defs>",
  "<linearGradient id='x' x1='0' x2='1' y1='0' y2='0'>",
  "<stop offset='0' stop-color='rgb(0,0,0)'/><stop offset='1' stop-color='rgb(255,0,0)'/>",
  "</linearGradient>",
  "<linearGradient id='y' x1='0' x2='0' y1='0' y2='1'>",
  "<stop offset='0' stop-color='rgb(0,0,0)'/><stop offset='1' stop-color='rgb(0,255,0)'/>",
  "</linearGradient>",
  "<filter id='b'><feGaussianBlur stdDeviation='14'/></filter>",
  "</defs>",
  "<rect width='200' height='200' fill='url(#x)'/>",
  "<rect width='200' height='200' fill='url(#y)' style='mix-blend-mode:screen'/>",
  "<rect x='26' y='26' width='148' height='148' rx='40' fill='rgb(128,128,0)' filter='url(#b)'/>",
  "</svg>",
].join("");

const MAP_URL = `data:image/svg+xml;utf8,${encodeURIComponent(MAP)}`;

export function GlassFilters() {
  return (
    <svg aria-hidden="true" width="0" height="0" style={{ position: "absolute" }}>
      <filter
        id="u-lens"
        x="0"
        y="0"
        width="100%"
        height="100%"
        filterUnits="objectBoundingBox"
        primitiveUnits="objectBoundingBox"
        colorInterpolationFilters="sRGB"
      >
        <feImage href={MAP_URL} x="0" y="0" width="1" height="1" preserveAspectRatio="none" result="map" />
        <feDisplacementMap
          in="SourceGraphic"
          in2="map"
          scale="0.08"
          xChannelSelector="R"
          yChannelSelector="G"
        />
      </filter>
    </svg>
  );
}
