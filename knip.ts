import type { KnipConfig } from "knip";

const config: KnipConfig = {
  // Entries come from the Vite (index.html), Vitest, Playwright and package.json script plugins;
  // standalone browser fixtures are loaded by their own HTML pages.
  entry: ["e2e/*.fixture.{ts,tsx}"],
  project: [
    "src/**/*.{ts,tsx,css}",
    "!src/editor/core/cm/angular/parser.terms.ts",
    "!src/editor/core/cm/jsp/parser.terms.ts",
    "!src/editor/core/cm/astro/parser.terms.ts",
    "scripts/**/*.ts",
    "perf/**/*.ts",
    "e2e/**/*.ts",
  ],
  ignoreExportsUsedInFile: true,
  // The strip left a large public surface no longer called from anywhere: editor
  // extension points, UI primitives and icon helpers the removed features used.
  // Removing them is safe but separate work; until then knip guards the issues that
  // break the build — unused files, dependencies and unresolved imports.
  rules: {
    exports: "off",
    types: "off",
    nsExports: "off",
    nsTypes: "off",
  },
  // lint-staged formats Rust files with the toolchain's rustfmt.
  ignoreBinaries: ["rustfmt"],
  compilers: {
    // Fonts come in through `@import` in global.css.
    css: (text: string) => [...text.matchAll(/(?<=@)import[^;]+/g)].join("\n"),
  },
};

export default config;
