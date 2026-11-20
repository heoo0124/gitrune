/**
 * Committing from the Commit tool window. Git is stubbed at the IPC layer; the repository side is
 * covered by `crates/ide-vcs` against real repositories.
 */
import type { Page } from "@playwright/test";
import { expect, ipcCalls, openFromTree, stubDesktop, test } from "./fixtures";

const ROOT = "/dev/acme-api";

/** A repository with a modified tracked file and an untracked one; `vcs_commit` clears both. */
async function stubRepository(page: Page) {
  await stubDesktop(page);
  await page.evaluate(async (root) => {
    let committed = false;
    const changes = [
      {
        path: `${root}/build.gradle.kts`,
        relPath: "build.gradle.kts",
        staged: null,
        unstaged: "modified",
        renamedFrom: null,
      },
      { path: `${root}/notes.txt`, relPath: "notes.txt", staged: null, unstaged: "untracked", renamedFrom: null },
    ];
    const h = window.__ipc.handlers;
    h.vcs_roots = () => [{ root, operation: null, head: "main", headCommit: "1111111" }];
    h.vcs_branch_info = () => ({
      head: "main",
      headCommit: "1111111",
      upstream: null,
      ahead: 0,
      behind: 0,
      operation: null,
    });
    h.vcs_status = () => (committed ? [] : changes);
    h.vcs_stage = () => null;
    h.vcs_unstage = () => null;
    h.vcs_commit = () => {
      committed = true;
      return "2222222";
    };
    const vcsModule = "/src/features/vcs/store.ts";
    const { useVcs, refreshNow } = await import(/* @vite-ignore */ vcsModule);
    useVcs.setState({ available: true, root, repo: root, branchesLoaded: false, stashesLoaded: false });
    await refreshNow();
  }, ROOT);
}

/** Switches the commit interface between the Git window's Local Changes and the Commit tool window. */
async function setNonModal(page: Page, nonModal: boolean) {
  await page.evaluate(async (value) => {
    const projectModule = "/src/platform/project.ts";
    const { useProject } = await import(/* @vite-ignore */ projectModule);
    const settings = useProject.getState().settings ?? {};
    const vcs = (settings as { vcs?: Record<string, unknown> }).vcs ?? {};
    const commit = (vcs as { commit?: Record<string, unknown> }).commit ?? {};
    useProject.setState({
      settings: { ...settings, vcs: { ...vcs, commit: { ...commit, nonModal: value } } } as never,
    });
  }, nonModal);
  // The tool window registry swaps the Commit window for the Local Changes tab on this change.
  await page.waitForTimeout(400);
}

test("non-modal: ⌘K commits the checked changes with the typed message", async ({ workbench: page }) => {
  await openFromTree(page, "build.gradle.kts");
  await stubRepository(page);
  await setNonModal(page, true);
  await page.keyboard.press("Meta+k");
  const panel = page.locator('[data-toolwindow="commit"]');
  const commit = panel.getByRole("button", { name: "Commit", exact: true });
  const changes = panel.getByRole("tree", { name: "Changes" });
  // Tracked changes start included; unversioned files start excluded.
  await expect(changes.getByRole("checkbox", { name: "Include build.gradle.kts in commit" })).toBeChecked();
  await expect(changes.getByRole("checkbox", { name: "Include Unversioned Files in commit" })).not.toBeChecked();
  const message = panel.getByRole("textbox", { name: "Commit message" });
  await expect(message).toBeFocused();

  // Committing without a message is refused.
  await commit.click();
  await expect(panel.getByRole("alert")).toHaveText("Specify commit message");
  expect(await ipcCalls(page, "vcs_commit")).toHaveLength(0);

  await message.fill("Return order DTOs from create");
  await commit.click();
  await expect(page.getByText(/1 file committed: Return order DTOs from create/)).toBeVisible();
  const [committed] = await ipcCalls(page, "vcs_commit");
  expect(committed).toMatchObject({ repo: ROOT, message: "Return order DTOs from create" });
  // The commit takes the checked file itself; the index isn't staged beforehand.
  expect((committed!.options as { stage: string[] }).stage).toEqual([`${ROOT}/build.gradle.kts`]);
  expect(await ipcCalls(page, "vcs_stage")).toHaveLength(0);
  await expect(changes.getByRole("checkbox", { name: "Include build.gradle.kts in commit" })).toBeHidden();
});

test("default: the commit interface lives in the Git window as Local Changes", async ({ workbench: page }) => {
  await openFromTree(page, "build.gradle.kts");
  await stubRepository(page);
  // IDEA's default: no separate Commit tool window; the Git window carries the changes.
  await expect(page.locator('[data-toolwindow="commit"]')).toHaveCount(0);
  await page.getByRole("button", { name: "Git", exact: true }).click();
  const git = page.locator('[data-toolwindow="git"]');
  await git.getByRole("tab", { name: "Local Changes", exact: true }).click();
  const changes = git.getByRole("tree", { name: "Changes" });
  await expect(changes.getByRole("treeitem", { name: /build\.gradle\.kts/ })).toBeVisible();
  // Unversioned files sit in their own (collapsed) group, as in IDEA.
  await expect(changes.getByRole("treeitem", { name: /Unversioned Files/ })).toBeVisible();
  // Local Changes has no inclusion checkboxes: the Commit dialog picks the files.
  expect(await changes.getByRole("checkbox").count()).toBe(0);
});
