/**
 * File icons for the project tree.
 *
 * The JVM presentation pass (Java/Kotlin class roles and Spring markers asked from the
 * language server) went with the code-development surface; icons now come from the file
 * name through `fileIconName`, which is what every other file used anyway.
 */
import { useFileIconRevision, fileIconName, type IconName } from "@/ui/icons";
import { basename } from "./project";

/** What the tree shows for a file: its icon, plus markers no language server fills now. */
export interface FilePresentation {
  icon: IconName;
  runnable: boolean;
}

/** No presentation cache is filled, so nothing is ever stale. */
export function invalidateFilePresentations(_paths: readonly string[]): void {}

/** The icon for `name` at `path`. */
export function presentationIcon(path: string, file?: FilePresentation | null, ..._rest: unknown[]): IconName {
  return file?.icon ?? fileIconName(basename(path), path);
}

/** The icon for a file, refreshed when the icon theme changes. */
export function useFilePresentation(path: string, _mtime?: number | null): IconName {
  const revision = useFileIconRevision();
  void revision;
  return fileIconName(basename(path), path);
}

/** No language server reports JVM roles, so no file is marked runnable. */
export function useJvmPresentation(path: string, _mtime?: number | null): FilePresentation {
  return { icon: useFilePresentation(path), runnable: false };
}

/** Icons for many paths (the tree asks for a visible slice at a time). */
export function useFilePresentationIcons(paths: readonly string[]): Map<string, IconName> {
  const revision = useFileIconRevision();
  void revision;
  const out = new Map<string, IconName>();
  for (const p of paths) out.set(p, fileIconName(basename(p), p));
  return out;
}
