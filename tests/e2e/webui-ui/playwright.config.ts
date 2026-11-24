import { defineConfig } from "@playwright/test";

// qa e2e · webui UI 旅程。服务器由 qa/run.py 起好并探活后才跑本套件：
// baseURL 取 QA_WEBUI_URL；浏览器可执行文件优先取 QA_CHROMIUM_PATH
// （系统 chromium，免下载），未设置时用 Playwright 自带浏览器。
export default defineConfig({
  testDir: "./tests",
  timeout: 30_000,
  retries: 0,
  reporter: [["list"], ["json", { outputFile: "../../../qa/.run/ui-results.json" }]],
  use: {
    baseURL: process.env.QA_WEBUI_URL ?? "http://127.0.0.1:19712",
    launchOptions: process.env.QA_CHROMIUM_PATH
      ? { executablePath: process.env.QA_CHROMIUM_PATH }
      : {},
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },
  outputDir: "../../../qa/.run/test-results",
});
