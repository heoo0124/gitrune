/**
 * Placeholders for the tree-sitter structure service editor/core used before GitRune
 * dropped syntax analysis: structural selection, move-statement and folding from the
 * parse tree.
 *
 * Every query answers "not available", so the actions that used them fall back to the
 * plain text behaviour they already had.
 */

export interface StructureQueries {
  [key: string]: unknown;
}

export interface StructuralRanges {
  [key: string]: unknown;
}

interface QueriesRequest {
  stamp: () => number;
  synced: () => Promise<unknown>;
  fetchRanges: (pos: unknown) => Promise<unknown>;
  fetchErrors: () => Promise<unknown>;
}

/** No parser is available, so a query handle is never produced. */
export function cachedStructureQueries(_request: QueriesRequest): StructureQueries {
  return {};
}

export function hasLezerTree(_view: unknown): boolean {
  return false;
}

export const structure = {
  nodeAt,
  async support(_language: string): Promise<{ tree: boolean }> {
    return { tree: false };
  },
  async structuralRanges(_doc: unknown, _pos: unknown): Promise<unknown> {
    return null;
  },
  async syntaxErrors(_doc: unknown): Promise<unknown> {
    return null;
  },
  queries: null as StructureQueries | null,
  ranges: null as StructuralRanges | null,
};

/** `"fallback"` tells the caller to use its plain text-editing path. */
export function moveStatementStructural(_view: unknown, _up: boolean, _queries: StructureQueries): Promise<"fallback"> {
  return Promise.resolve("fallback");
}

export function completeStatementStructural(
  _view: unknown,
  _language: string,
  _queries: StructureQueries,
): Promise<"fallback"> {
  return Promise.resolve("fallback");
}

/** No parse tree is available, so a node at a position never resolves. */
export async function nodeAt(
  _doc: unknown,
  _range: unknown,
): Promise<{
  node: {
    kind: string;
    range: { start: { line: number; col: number }; end: { line: number; col: number } };
    field?: string;
  };
  ancestors: Array<{
    kind: string;
    range: { start: { line: number; col: number }; end: { line: number; col: number } };
    field?: string;
  }>;
} | null> {
  return null;
}
