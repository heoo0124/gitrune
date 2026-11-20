/**
 * Placeholders for the language-server helpers that editor/core used before GitRune
 * dropped code intelligence (go-to-definition, format-on-save, the LSP list popup).
 *
 * The editor's own actions still reference them; each one reports "nothing available"
 * so the action is a no-op rather than an error.
 */
import { create } from "zustand";

export type LspPopup =
  | {
      kind: "list";
      title?: string;
      anchor?: unknown;
      items: Array<{ id: string; label: string; [key: string]: unknown }>;
      [key: string]: unknown;
    }
  | { kind: string; [key: string]: unknown };

/** The list popup the LSP actions drove; nothing opens it now. */
export const useLspPopup = create<{ popup: LspPopup | null }>(() => ({ popup: null }));

/** The list popup's items, for callers that patch one entry. */
export type LspPopupItem = { id: string; label: string; [key: string]: unknown };

export function caretAnchor(_view?: unknown, _head?: number): { top: number; left: number } {
  return { top: 0, left: 0 };
}

export function openPopup(popup: LspPopup): void {
  useLspPopup.setState({ popup });
}

export function updatePopup(kind: string, patch: Record<string, unknown>): void {
  useLspPopup.setState((state) => (state.popup ? { popup: { ...state.popup, ...patch, kind } } : state));
}

/** No language server answers, so every request rejects. */
export function lspRequest<T>(_fn: () => Promise<T>): Promise<T> {
  return Promise.reject(new Error("no language server is running"));
}

export async function withDocument<T>(_path: string, _fn: (doc: unknown) => Promise<T>): Promise<T | null> {
  return null;
}

export async function readFileLines(_path: string): Promise<string[] | null> {
  return null;
}

export function formatDocument(_view?: unknown, _options?: unknown): Promise<{ applied: boolean }> {
  return Promise.resolve({ applied: false });
}

/** Offsets in the open document; the LSP client used to own these. */
type TextLike = string | { toString(): string; line(n: number): { from: number } };

export function offsetToPos(text: TextLike, offset: number): { line: number; col: number } {
  const before = text.toString().slice(0, offset).split("\n");
  return { line: before.length - 1, col: before[before.length - 1]!.length };
}

export function posToOffset(text: TextLike, pos: { line: number; col: number }): number {
  const lines = text.toString().split("\n");
  let offset = 0;
  for (let i = 0; i < pos.line && i < lines.length; i++) offset += lines[i]!.length + 1;
  return offset + pos.col;
}

/** The declaration chain above a position; no parser means no chain. */
export function declarationChain(
  _outline: unknown,
  _line: number,
  _col: number,
): Array<{ name: string; kind: string }> {
  return [];
}

/** A range between two UTF-16 offsets, expressed as the LSP positions editors use. */
export function offsetsToRange(doc: TextLike, from: number, to: number) {
  return { start: offsetToPos(doc, from), end: offsetToPos(doc, to) };
}

/** The list-popup item shape some vcs callers still name. */
export type ListPopupItem = { id: string; label: string; [key: string]: unknown };

/** Backend diagnostics are gone; nothing is annotated. */
export function setFileDiagnostics(..._args: unknown[]): void {}

/** Per-document formatter options; no language server means no options. */
export function formatOptionsOf(..._args: unknown[]): unknown {
  return null;
}
