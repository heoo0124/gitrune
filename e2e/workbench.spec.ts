import { expect, PROJECT, test } from "./fixtures";

test("renders title bar, stripes, tool window and status bar", async ({ workbench: page }) => {
  const title = page.locator("header").first();
  await expect(title.getByRole("button", { name: PROJECT })).toBeVisible();
  await expect(title.getByRole("button", { name: "IDE and Project Settings", exact: true })).toBeVisible();

  const left = page.getByRole("toolbar", { name: "Tool windows" });
  // The default (modal) commit interface keeps Commit inside the Git window, so the
  // left stripe carries Project and the bottom stripe carries Git.
  await expect(left.getByRole("button", { name: "Project", exact: true })).toBeVisible();
  // Git is docked at the bottom and starts collapsed; only Project is open at launch.
  await expect(page.locator('[data-toolwindow="project"]')).toBeVisible();
  await expect(page.getByRole("toolbar", { name: "Right tool windows" })).toBeVisible();

  await expect(page.getByRole("region", { name: "Project" })).toBeVisible();
  const status = page.getByRole("contentinfo", { name: "Status bar" });
  await expect(status).toBeVisible();
  // The stripes stay even with an empty editor area.
  await expect(page.getByRole("toolbar", { name: "Tool windows", exact: true })).toBeVisible();
});

test("stripe buttons toggle tool windows", async ({ workbench: page }) => {
  const stripe = page.getByRole("toolbar", { name: "Tool windows" });
  const project = page.locator('[data-toolwindow="project"]');
  await expect(project).toBeVisible();
  await stripe.getByRole("button", { name: "Project", exact: true }).click();
  await expect(project).toBeHidden();
  await stripe.getByRole("button", { name: "Project", exact: true }).click();
  await expect(project).toBeVisible();
});

for (const reducedMotion of ["no-preference", "reduce"] as const) {
  test(`project opening advances simulated progress with ${reducedMotion} motion`, async ({ workbench: page }) => {
    await page.emulateMedia({ reducedMotion });
    await page.evaluate(async () => {
      const path = "/src/platform/project.ts";
      const { useProject } = await import(/* @vite-ignore */ path);
      useProject.setState({ opening: true, openingPath: "/work/large-project" });
    });
    const progress = page.locator("[data-project-opening]").getByRole("progressbar");
    await expect(progress).toBeVisible();
    for (const theme of ["light", "dark"]) {
      await page.evaluate((theme) => (document.documentElement.dataset.theme = theme), theme);
      const frames = await progress.evaluate((track) => {
        const fill = track.firstElementChild!;
        const animation = fill.getAnimations()[0];
        if (!animation) throw new Error("Loading progress has no activity indicator");
        animation.pause();
        const duration = Number(animation.effect!.getComputedTiming().duration);
        return [0, 0.25, 0.5, 0.75, 1, 1.5, 2].map((phase) => {
          animation.currentTime = duration * phase;
          const outer = track.getBoundingClientRect();
          const inner = fill.getBoundingClientRect();
          const style = getComputedStyle(fill);
          return {
            trackWidth: outer.width,
            fillWidth: inner.width,
            height: inner.height,
            left: inner.left - outer.left,
            right: outer.right - inner.right,
            opacity: Number(style.opacity),
            color: style.backgroundColor,
          };
        });
      });
      for (const frame of frames) {
        expect(frame.height).toBeGreaterThan(0);
        expect(frame.fillWidth).toBeGreaterThan(0);
        expect(frame.fillWidth).toBeLessThan(frame.trackWidth);
        expect(frame.left).toBeGreaterThanOrEqual(-0.5);
        expect(frame.right).toBeGreaterThanOrEqual(-0.5);
        expect(frame.opacity).toBeGreaterThanOrEqual(0.6);
        expect(frame.color).not.toBe("rgba(0, 0, 0, 0)");
      }
      const widths = frames.map((frame) => frame.fillWidth);
      expect(widths[0]).toBeLessThan(widths[1]!);
      for (let i = 1; i < widths.length; i++) expect(widths[i]).toBeGreaterThanOrEqual(widths[i - 1]!);
      expect(widths[4]! / frames[4]!.trackWidth).toBeCloseTo(0.92, 2);
      expect(widths[6]).toBeCloseTo(widths[4]!, 2);
    }
  });
}
