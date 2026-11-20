/**
 * Feature activation. Each feature exports `activate(): void | (() => void)`
 * that registers actions, tool windows, status items and services, returning
 * a disposer. The editor provides the editor service; app must install its launch
 * barrier before welcome can restore a remembered project.
 *
 * GitRune keeps the version-control surface: the Git UI, GitHub pull requests,
 * the editor the diff views run in, the file tree, settings and the welcome screen.
 */
import { activate as editor } from "@/editor/core";
import { activate as welcome } from "./welcome";
import { activate as projectTree } from "./project-tree";
import { activate as vcs, activateComparisonWindow, activateSettingsPages as vcsPages } from "./vcs";
import { activate as settings, activateRuntimeSettings, activateSettingsWindow } from "./settings";
import { activate as app, activateLocale as appLocale, activateMenuBridge, confirmLeave } from "./app";
import { activate as projectTrust } from "./project-trust";
import { activate as github, activateSettingsPages as githubPages } from "./github";

type Activate = () => void | (() => void);

const features: Array<[string, Activate]> = [
  // Before anything opens a project: the trust prompt decides what it may run.
  ["project-trust", projectTrust],
  ["editor", editor],
  ["app", app],
  ["welcome", welcome],
  ["project-tree", projectTree],
  ["vcs", vcs],
  ["github", github],
  ["settings", settings],
];

function activateAll(what: string, list: Array<[string, Activate]>): () => void {
  const disposers: Array<() => void> = [];
  for (const [name, activate] of list) {
    try {
      const d = activate();
      if (d) disposers.push(d);
    } catch (e) {
      console.error(`${what} of ${name} failed to activate`, e);
    }
  }
  return () => disposers.reverse().forEach((d) => d());
}

export function activateFeatures(): () => void {
  return activateAll("feature", features);
}

export { SettingsWindow } from "./settings";
export { ComparisonWindow } from "./vcs";

/**
 * A comparison window of project window `parent`: the changed files between two revisions and
 * their diffs, with the editor and Git but no workbench.
 */
export function activateComparisonWindowFeatures(parent: string): () => void {
  return activateAll("comparison window", [
    ["locale", appLocale],
    ["settings", activateRuntimeSettings],
    ["menu", activateMenuBridge],
    ["editor", editor],
    ["vcs", vcs],
    ["comparison", () => activateComparisonWindow(parent, (project) => confirmLeave(undefined, project))],
  ]);
}

/**
 * The native Settings window of project window `parent`: only the settings
 * pages (no workbench, project, editors or background services).
 */
export function activateSettingsWindowFeatures(parent: string): () => void {
  const pages: Array<[string, Activate]> = [
    ["locale", appLocale],
    ["settings", () => activateSettingsWindow(parent)],
    ["vcs", vcsPages],
    ["github", githubPages],
  ];
  return activateAll("settings pages", pages);
}
