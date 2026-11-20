/**
 * A minimal code preview for the bookmarks popup.
 *
 * The original `parts.tsx` belonged to Find in Files (sessions, preview, replace
 * flow). GitRune only needs the small preview pane, so it renders the text of the
 * bookmarked file without the search-specific machinery.
 */
import { useEffect, useState } from "react";
import { call, commands } from "@/platform/ipc";
import styles from "./parts.module.css";

export function CodePreview({
  path,
  line,
  highlights,
}: {
  path: string | null;
  line: number;
  highlights: readonly unknown[];
}) {
  void highlights;
  const [text, setText] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    if (!path) return;
    void call(commands.fsReadText(path))
      .then((t) => {
        if (live) setText(typeof t === "string" ? t : null);
      })
      .catch(() => {
        if (live) setText(null);
      });
    return () => {
      live = false;
    };
  }, [path]);
  const lines = text?.split("\n") ?? [];
  const from = Math.max(0, line - 3);
  const shown = lines.slice(from, from + 12);
  return (
    <div className={styles.preview}>
      {shown.map((l, i) => (
        <div key={from + i} className={from + i === line ? styles.current : undefined}>
          <span className={styles.lineNumber}>{from + i + 1}</span>
          <span>{l}</span>
        </div>
      ))}
    </div>
  );
}
