/**
 * Read-only build context shared by file and folder presentation consumers.
 *
 * GitRune has no Maven/Gradle model (the build tooling went with the rest of the
 * code-development surface), so every folder presents as an ordinary folder and no
 * Spring module is highlighted.
 */
import { create } from "zustand";
import type { BuildModel } from "./ipc";

export interface BuildPresentation {
  modules: ReadonlyMap<string, "maven" | "gradle">;
  spring: boolean;
}
const EMPTY: BuildPresentation = { modules: new Map(), spring: false };
export const useBuildPresentation = create<BuildPresentation>(() => EMPTY);

export function buildPresentation(_model: BuildModel): BuildPresentation {
  return EMPTY;
}

/** A Maven / Gradle module folder below the project root: IDEA shows its name in bold. */
export function isModuleFolder(_rel: string, _root: boolean, _modules: BuildPresentation["modules"]): boolean {
  return false;
}

/** Build presentations are never loaded; the subscription API stays for its consumers. */
export function useBuildPresentationFor(_root: string | null): BuildPresentation {
  return EMPTY;
}

/** The presentation for an open project: no build model means no module folders. */
export function useProjectPresentation(_root?: string | null): BuildPresentation {
  return EMPTY;
}

/** The icon for a module folder; no build model means no special icon. */
export function moduleFolderIcon(..._args: unknown[]): string | null {
  return null;
}
