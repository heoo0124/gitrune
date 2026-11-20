import { expect, test } from "./fixtures";

test("⌘, opens Settings with the page tree; Escape closes", async ({ workbench: page }) => {
  await page.keyboard.press("Meta+,");
  const dialog = page.getByRole("dialog", { name: "Settings" });
  await expect(dialog).toBeVisible();
  const item = (name: string) => dialog.getByRole("treeitem", { name, exact: true });
  for (const name of ["Appearance & Behavior", "Keymap", "Editor"]) {
    await expect(item(name)).toBeVisible();
  }
  // The tree is virtualized: collapse Editor (expanded by default) so the later groups render.
  await item("Editor").click();
  await page.keyboard.press("ArrowLeft");
  await expect(item("Editor")).not.toHaveAttribute("aria-expanded", "true");
  await expect(item("Version Control")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
});

test("font picker supports manual families and CJK fallback in browser development", async ({ workbench: page }) => {
  await page.keyboard.press("Meta+,");
  const dialog = page.getByRole("dialog", { name: "Settings" });
  await dialog.getByRole("textbox", { name: "Search settings" }).fill("Typography Settings");
  await dialog.getByRole("treeitem", { name: "Font", exact: true }).click();
  const family = dialog.getByRole("combobox", { name: "Font", exact: true });
  await family.click();
  await expect(page.getByRole("option", { name: /JetBrains Mono/ })).toBeVisible();
  await family.fill("Maple Mono NF");
  await family.press("Enter");
  await expect(family).toHaveValue("Maple Mono NF");
  // The shipped face is Inter Variable, not a system-installed family named Inter.
  const interLoaded = await page.evaluate(async () => {
    const faces = await document.fonts.load('14px "Inter Variable"');
    return faces.some((face) => face.family.includes("Inter Variable") && face.status === "loaded");
  });
  expect(interLoaded).toBe(true);
  await family.fill("Inter");
  await family.press("Enter");
  await expect(dialog.getByLabel("Font preview")).toHaveCSS("font-family", /^"Inter Variable",/);
  const fallback = dialog.getByRole("combobox", { name: "Fallback font", exact: true });
  await family.fill("Maple Mono NF");
  await family.press("Enter");
  await fallback.fill("Inter");
  await fallback.press("Enter");
  await expect(dialog.getByLabel("Font preview")).toHaveCSS("font-family", /Maple Mono NF.*Inter Variable/);
  await fallback.fill("思源宋体");
  await fallback.press("Enter");
  await expect(dialog.getByLabel("Font preview")).toHaveCSS("font-family", /思源宋体/);
  await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(dialog).toBeHidden();
});
