import { readFileSync, existsSync } from "node:fs";
import { join } from "node:path";

const root = new URL(".", import.meta.url).pathname;
const htmlPath = join(root, "index.html");
const cssPath = join(root, "styles.css");
const jsPath = join(root, "main.js");
const faviconPath = join(root, "favicon.svg");

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

assert(existsSync(htmlPath), "website/index.html must exist");
assert(existsSync(cssPath), "website/styles.css must exist");
assert(existsSync(jsPath), "website/main.js must exist");
assert(existsSync(faviconPath), "website/favicon.svg must exist");

const html = readFileSync(htmlPath, "utf8");
const css = readFileSync(cssPath, "utf8");
const js = readFileSync(jsPath, "utf8");

const requiredCopy = [
  "Forge",
  "A desktop command center",
  "for AI coding agents.",
  "Private by default",
  "Review Execution",
  "Visibility at every level.",
  "Structured Traces",
  "Local Toolkit",
  "TERMINAL-GRADE AGENTS. DESKTOP-GRADE CONTROL.",
];

for (const copy of requiredCopy) {
  assert(html.includes(copy), `Missing required copy: ${copy}`);
}

assert(
  html.includes('<span>A desktop command center</span>') &&
    html.includes("<span>for AI coding agents.</span>"),
  "Hero headline must be authored as two explicit lines",
);

assert(css.includes("@media"), "styles.css must include responsive media queries");
assert(css.includes("Inter"), "styles.css must define the headline/brand font");
assert(css.includes("JetBrains Mono"), "styles.css must define the technical mono font");
assert(html.includes("gsap@3.13.0"), "index.html must load a pinned GSAP version");
assert(html.includes("favicon.svg"), "index.html must define a local favicon");
assert(html.includes("ScrollTrigger.min.js"), "index.html must load ScrollTrigger");
assert(js.includes("prefers-reduced-motion"), "main.js must respect reduced motion");
assert(js.includes("gsap.timeline"), "main.js must use a sequenced hero timeline");
assert(js.includes("ScrollTrigger"), "main.js must define scroll-triggered motion");
assert(js.includes("dataset.motion"), "main.js must expose motion state for QA");

console.log("website verification passed");
