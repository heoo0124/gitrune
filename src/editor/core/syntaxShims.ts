/**
 * Placeholder for the tree-sitter highlighter editor/core used before GitRune dropped
 * syntax analysis. CodeMirror's own language modes do the highlighting; the backend's
 * tree-sitter pass is gone, so these report that nothing is available.
 */
export type SyntaxHighlights = { kinds: string[]; data: number[] };
export type SyntaxHighlightKind = string;

export function hasHighlights(): boolean {
  return false;
}

export function createSyntaxHighlighter(): null {
  return null;
}

export function syntaxHighlightExtension(): unknown[] {
  return [];
}

import { StateEffect } from "@codemirror/state";
import type { Extension } from "@codemirror/state";

/** Backend tree-sitter highlights never arrive now, but the state effect keeps its shape. */
export const setSyntaxHighlights = StateEffect.define<SyntaxHighlights>();

/** A no-op CodeMirror extension; the backend highlighter used to install one. */
export const syntaxHighlightField: Extension = [];
