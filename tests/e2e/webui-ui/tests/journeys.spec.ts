import { copyFileSync, rmSync } from "node:fs";
import path from "node:path";
import { expect, test } from "@playwright/test";

// e2e · webui UI 旅程。断言只落在用户可见行为上（文件树、徽章、过滤、预览）。
// 前置：qa/run.py 已构建产物、起好服务器并通过 /api/health 探活。
// 夹具仓库：a.xlsx 干净 · b.xlsx modified · c_untracked untracked · d_err.xlsx 含 1 个越界错误。

const FIXTURE_DATA = path.join(process.env.QA_FIXTURE_REPO ?? "", "data");
const LIVE_FILE = path.join(FIXTURE_DATA, "e_live.xlsx");

test.describe("webui 核心旅程", () => {
  test("文件树加载 → 选择文件 → 预览渲染", async ({ page }) => {
    await page.goto("/");

    // 文件树出现夹具中的文件行（Lit open shadow root，Playwright 自动穿透）
    const rail = page.locator("file-list");
    await expect(rail).toBeVisible({ timeout: 15_000 });
    await expect(rail.locator(".row", { hasText: "a.xlsx" })).toBeVisible({ timeout: 15_000 });
    await expect(rail.locator(".row", { hasText: "c_untracked.xlsx" })).toBeVisible();

    // 点击叶子行 → 预览面板渲染出该表内容（a.xlsx 的数据值）
    await rail.locator(".row", { hasText: "a.xlsx" }).click();
    const preview = page.locator("file-preview");
    await expect(preview).toBeVisible();
    await expect(preview.getByText("alpha", { exact: false }).first()).toBeVisible({
      timeout: 15_000,
    });
  });

  test("Check 结果 → 文件行错误徽章 → modified-only 过滤收窄列表", async ({ page }) => {
    await page.goto("/");

    const rail = page.locator("file-list");
    await expect(rail.locator(".row", { hasText: "d_err.xlsx" })).toBeVisible({
      timeout: 15_000,
    });

    // 右侧 build-panel 的 Check 按钮：输出区列出诊断
    const checkBtn = page.locator("build-panel wa-button[data-kind='check']");
    await expect(checkBtn).toBeVisible();
    await checkBtn.click();
    await expect(
      page.locator("build-panel").getByText("ValueOutOfRange").first(),
    ).toBeVisible({ timeout: 20_000 });

    // 文件行的健康徽章来自每次 listing 之后的自动 check（api.ts scheduleCheck），
    // 首屏即有，无需 Reload：d_err 行亮红色徽章 1
    const errBadge = rail
      .locator(".row", { hasText: "d_err.xlsx" })
      .locator(".badge");
    await expect(errBadge).toBeVisible({ timeout: 20_000 });
    await expect(errBadge).toHaveText("1");

    // modified-only 过滤：只剩 b.xlsx / c_untracked.xlsx，a 与 d_err 不可见
    await rail.locator(".filter button", { hasText: "Modified only" }).click();
    await expect(rail.locator(".row", { hasText: "b.xlsx" })).toBeVisible({
      timeout: 15_000,
    });
    await expect(rail.locator(".row", { hasText: "c_untracked.xlsx" })).toBeVisible();
    await expect(rail.locator(".row", { hasText: "a.xlsx" })).toHaveCount(0);
    await expect(rail.locator(".row", { hasText: "d_err.xlsx" })).toHaveCount(0);
  });

  test("新增文件 → 实时通知 → 列表自动刷新，且空闲与刷新都不循环", async ({ page }) => {
    // 监听器必须先于页面加载挂上，才能看到首屏建立的那条 WebSocket。
    let notifications = 0;
    page.on("websocket", (ws) => {
      ws.on("framereceived", (frame) => {
        if (String(frame.payload) === "files_changed") notifications += 1;
      });
    });

    try {
      await page.goto("/");
      const rail = page.locator("file-list");
      await expect(rail.locator(".row", { hasText: "a.xlsx" })).toBeVisible({
        timeout: 15_000,
      });

      // 空闲：服务器自己的扫描不是变更，不该有任何通知
      await page.waitForTimeout(1500);
      expect(notifications, "an idle directory must not notify").toBe(0);

      // 往被监控目录放入一个新表 → 不刷新页面，文件树自己出现它
      copyFileSync(path.join(FIXTURE_DATA, "a.xlsx"), LIVE_FILE);
      await expect(rail.locator(".row", { hasText: "e_live.xlsx" })).toBeVisible({
        timeout: 15_000,
      });

      // 一次真实改动只通知一次；通知引发的刷新不会再触发下一轮
      await page.waitForTimeout(2500);
      expect(notifications, "one real change must notify exactly once").toBe(1);
    } finally {
      rmSync(LIVE_FILE, { force: true });
    }
  });
});
