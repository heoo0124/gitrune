/**
 * File move / rename plumbing for the project tree.
 *
 * GitRune has no reference-updating refactoring (that needed the language servers), so
 * a move or rename only touches the disk through `commands.refactorMoveFiles`, and
 * there is no undo stack beyond the file system's own.
 */
import { projectFs } from "./fs";

export interface FileMove {
  from: string;
  to: string;
}

export interface RefactorOptions {
  openInEditor?: boolean;
  [key: string]: unknown;
}

const REMEMBERED = new Map<string, RefactorOptions>();
let last: (FileMove[] & { seq: number }) | null = null;

export function rememberedOptions(kind: string): RefactorOptions {
  return REMEMBERED.get(kind) ?? {};
}

export function rememberOptions(kind: string, options: RefactorOptions): void {
  REMEMBERED.set(kind, options);
}

export function lastFileRefactoring(): (FileMove[] & { seq: number }) | null {
  return last;
}

/** Moves or renames `moves` on disk; resolves to `"done"` when they all applied. */
export async function runFileRefactoring(
  root: string,
  moves: FileMove[],
  _options: RefactorOptions,
  _label: string,
): Promise<"done" | "failed"> {
  try {
    const fs = projectFs(root);
    for (const m of moves) await fs.rename(m.from, m.to);
    last = Object.assign([...moves], { seq: Date.now() });
    return "done";
  } catch {
    return "failed";
  }
}

export async function undoFileRefactoring(root: string): Promise<void> {
  if (!last) return;
  const back = last.map((m) => ({ from: m.to, to: m.from }));
  try {
    const fs = projectFs(root);
    for (const m of back) await fs.rename(m.from, m.to);
  } catch {
    // The disk is the source of truth; a failed undo leaves the tree as it is.
  }
  last = null;
}

/** A monotonically increasing id for file operations, so Delete and Move undo in order. */
let operationSeq = 0;
export function nextFileOperation(): number {
  return ++operationSeq;
}
