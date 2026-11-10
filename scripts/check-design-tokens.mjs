import { readFileSync } from "node:fs";
import assert from "node:assert/strict";
const css = readFileSync(
  new URL("../src/lib/styles/system.css", import.meta.url),
  "utf8",
);
const properties = (block) =>
  Object.fromEntries(
    [...block.matchAll(/(--[\w-]+):\s*(#[\da-f]+)/gi)].map((m) => [m[1], m[2]]),
  );
const light = properties(css.match(/:root\s*\{([^}]+)\}/)[1]);
const dark = {
  ...light,
  ...properties(css.match(/:root\[data-theme="dark"\]\s*\{([^}]+)\}/)[1]),
};
const highContrast = css.slice(css.indexOf("@media (prefers-contrast: more)"));
const highLight = {
  ...light,
  ...properties(highContrast.match(/:root\s*\{([^}]+)\}/)[1]),
};
const highDark = {
  ...dark,
  ...properties(
    highContrast.match(/:root\[data-theme="dark"\]\s*\{([^}]+)\}/)[1],
  ),
};
function luminance(hex) {
  hex = hex.slice(1);
  if (hex.length === 3) hex = [...hex].map((c) => c + c).join("");
  const c = hex
    .match(/../g)
    .map((x) => parseInt(x, 16) / 255)
    .map((x) => (x <= 0.04045 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4));
  return c[0] * 0.2126 + c[1] * 0.7152 + c[2] * 0.0722;
}
const contrast = (a, b) =>
  (Math.max(luminance(a), luminance(b)) + 0.05) /
  (Math.min(luminance(a), luminance(b)) + 0.05);
for (const [name, theme] of Object.entries({
  light,
  dark,
  highLight,
  highDark,
})) {
  for (const bg of ["--background", "--surface", "--muted"])
    for (const fg of ["--foreground", "--muted-foreground", "--destructive"])
      assert.ok(
        contrast(theme[fg], theme[bg]) >= 4.5,
        `${name}: ${fg} on ${bg} must meet 4.5:1`,
      );
  for (const bg of ["--background", "--surface"])
    assert.ok(
      contrast(theme["--control-border"], theme[bg]) >= 3,
      `${name}: control border must meet 3:1`,
    );
  for (const bg of ["--primary-foreground", "--accent-soft"])
    assert.ok(
      contrast(theme["--primary"], theme[bg]) >= 4.5,
      `${name}: primary text on ${bg} must meet 4.5:1`,
    );
  assert.ok(
    contrast(theme["--action"], theme["--action-foreground"]) >= 4.5,
    `${name}: action labels must meet 4.5:1`,
  );
  console.log(
    `${name}: text >=4.5:1; control boundaries >=3:1; primary ${contrast(theme["--primary"], theme["--primary-foreground"]).toFixed(2)}:1`,
  );
}
