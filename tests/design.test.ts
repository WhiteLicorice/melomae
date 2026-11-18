import { describe, expect, it } from "vitest";

import designSource from "../DESIGN.md?raw";

type Theme = "light" | "dark";

interface ContrastPair {
  label: string;
  foreground: string;
  background: string;
  minimum: number;
}

const THEMES: Theme[] = ["light", "dark"];
const THEME_SUFFIX = /-(light|dark)$/;

function readFrontmatter(source: string): string {
  const match = /^---\r?\n([\s\S]*?)\r?\n---\r?\n/.exec(source);
  if (match === null) {
    throw new Error("DESIGN.md has no YAML frontmatter.");
  }
  return match[1];
}

function readColorTokens(source: string): Map<string, string> {
  const block = /^colors:\r?\n((?:[ \t]+\S.*\r?\n)+)/m.exec(
    readFrontmatter(source),
  );
  if (block === null) {
    throw new Error("DESIGN.md has no color tokens in its frontmatter.");
  }

  const tokens = new Map<string, string>();
  for (const line of block[1].split(/\r?\n/)) {
    const entry = /^[ \t]+([a-z0-9-]+):\s*"(#[0-9a-fA-F]{6})"\s*$/.exec(line);
    if (entry !== null) {
      tokens.set(entry[1], entry[2].toLowerCase());
    }
  }
  return tokens;
}

function readContrastPairs(source: string): ContrastPair[] {
  const start = source.indexOf("### Contrast pairs");
  if (start === -1) {
    throw new Error("DESIGN.md has no '### Contrast pairs' table.");
  }

  const pairs: ContrastPair[] = [];
  for (const line of source.slice(start).split(/\r?\n/).slice(1)) {
    if (line.startsWith("## ")) {
      break;
    }
    if (!line.startsWith("|")) {
      continue;
    }
    const cells = line.split("|").map((cell) => cell.trim());
    if (cells.length < 6) {
      continue;
    }
    const [, label, foreground, background, minimum] = cells;
    if (label === "Pair" || /^-+$/.test(label)) {
      continue;
    }
    const required = Number(minimum);
    if (!Number.isFinite(required)) {
      throw new Error(`Contrast pair has no numeric minimum: ${line}`);
    }
    pairs.push({ label, foreground, background, minimum: required });
  }
  return pairs;
}

function luminance(hex: string): number {
  const channel = (offset: number): number => {
    const value = parseInt(hex.slice(1 + offset, 3 + offset), 16) / 255;
    return value <= 0.03928 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * channel(0) + 0.7152 * channel(2) + 0.0722 * channel(4);
}

function contrast(foreground: string, background: string): number {
  const first = luminance(foreground);
  const second = luminance(background);
  const [bright, dark] = first > second ? [first, second] : [second, first];
  return (bright + 0.05) / (dark + 0.05);
}

describe("DESIGN.md", () => {
  const tokens = readColorTokens(designSource);
  const pairs = readContrastPairs(designSource);

  it("computes the WCAG ratio of two known pairs", () => {
    expect(contrast("#000000", "#ffffff")).toBeCloseTo(21, 2);
    expect(contrast("#777777", "#ffffff")).toBeCloseTo(4.48, 2);
  });

  it("gives every color token a light value and a dark value", () => {
    const names = new Set(
      [...tokens.keys()].map((token) => token.replace(THEME_SUFFIX, "")),
    );

    expect(names.size).toBeGreaterThan(0);
    for (const name of names) {
      expect(tokens.has(`${name}-light`), `${name} has no light value.`).toBe(
        true,
      );
      expect(tokens.has(`${name}-dark`), `${name} has no dark value.`).toBe(
        true,
      );
    }
  });

  it("requires 3:1 for the control edge and the focus ring", () => {
    for (const pair of pairs) {
      if (pair.foreground === "control-edge" || pair.foreground === "focus") {
        expect(pair.minimum, `${pair.label} must require 3:1.`).toBe(3);
      }
    }

    const foregrounds = pairs.map((pair) => pair.foreground);
    expect(foregrounds).toContain("control-edge");
    expect(foregrounds).toContain("focus");
  });

  it.each(THEMES)("meets every stated minimum in the %s theme", (theme) => {
    expect(pairs.length).toBeGreaterThan(0);

    for (const pair of pairs) {
      const foreground = tokens.get(`${pair.foreground}-${theme}`);
      const background = tokens.get(`${pair.background}-${theme}`);
      if (foreground === undefined || background === undefined) {
        throw new Error(
          `${pair.label} in the ${theme} theme names a token that DESIGN.md does not define.`,
        );
      }

      const ratio = contrast(foreground, background);
      expect(
        ratio,
        `${pair.label} in the ${theme} theme: ${pair.foreground} on ${pair.background} is ${ratio.toFixed(2)}:1 and the table requires ${pair.minimum}:1.`,
      ).toBeGreaterThanOrEqual(pair.minimum);
    }
  });
});
