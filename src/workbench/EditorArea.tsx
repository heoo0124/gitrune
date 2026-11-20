import { executeAction, useActions } from "@/platform/actions";
import { shortcutLabel } from "@/platform/keymap";
import { useProject } from "@/platform/project";
import { T } from "@/ui";
import { useWorkbenchSlots } from "./contributions";
import { ErrorBoundary, Slot } from "./ErrorBoundary";
import styles from "./EditorArea.module.css";

type Hint = { label: string; action?: string; fallback?: string };

// The Git client's everyday entries. Every action here is one GitRune ships:
// an editor hint that resolves to nothing reads as a broken shortcut.
const HINTS: Hint[] = [
  { label: "Commit…", action: "vcs.commit" },
  { label: "Update Project", action: "vcs.update" },
  { label: "Show Git Log", action: "vcs.showLog" },
  { label: "Branches", action: "vcs.branches" },
  { label: "Local History", action: "vcs.localHistory" },
  { label: "Drop files here to open them" },
];

/** LightEdit has no project to navigate: only opening files works there. */
const LIGHT_EDIT_HINTS: Hint[] = [
  { label: "Open File", action: "file.openFile" },
  { label: "Drop files here to open them" },
];

/** IDEA's empty editor: a few navigation hints with their shortcuts. */
export function EmptyEditor() {
  const actions = useActions((s) => s.actions);
  const lightEdit = useProject((s) => s.lightEdit);
  return (
    <div className={styles.empty}>
      <dl className={styles.hints}>
        {(lightEdit ? LIGHT_EDIT_HINTS : HINTS).map((h) => {
          const keys = h.action ? (shortcutLabel(h.action) ?? h.fallback) : undefined;
          const runnable = !!h.action && actions.has(h.action);
          return (
            <div key={h.label} className={styles.hint}>
              <dt>
                {runnable ? (
                  <button type="button" className={styles.hintButton} onClick={() => void executeAction(h.action!)}>
                    <T>{h.label}</T>
                  </button>
                ) : (
                  <T>{h.label}</T>
                )}
              </dt>
              {keys && <dd className={styles.keys}>{keys}</dd>}
            </div>
          );
        })}
      </dl>
    </div>
  );
}

export function EditorArea() {
  const Provided = useWorkbenchSlots((s) => s.editorArea);
  return (
    <div className={styles.area} data-editor-area="" tabIndex={-1}>
      {Provided ? (
        <ErrorBoundary name="editor area" fallback={<EmptyEditor />}>
          <Slot component={Provided} />
        </ErrorBoundary>
      ) : (
        <EmptyEditor />
      )}
    </div>
  );
}
