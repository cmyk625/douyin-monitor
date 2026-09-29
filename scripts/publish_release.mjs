import { execSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

const TAG = "v0.1.0";
const RELEASE_NAME = "Douyin Monitor Desktop v0.1.0";
const REPO = "cmyk625/douyin-monitor";

console.log(">>> 获取 Git Credential 认证 Token...");
const credOut = execSync('git credential fill', {
  input: "protocol=https\nhost=github.com\n",
  encoding: "utf-8"
});
const match = credOut.match(/password=(.+)/);
if (!match || !match[1]) {
  console.error("未找到 GitHub Token");
  process.exit(1);
}
const token = match[1].trim();

const headers = {
  Authorization: `Bearer ${token}`,
  Accept: "application/vnd.github+json",
  "User-Agent": "douyin-monitor-publisher"
};

const releaseBody = `## 抖音作品数据监控 (Douyin Monitor Desktop) ${TAG}

### 🚀 核心特性与更新亮点
- 🎯 **指定目标精准监控**：支持抖音 App 完整分享口令与 PC 网页长链，自动清洗提取规范作品链接。
- 📊 **四维互动实时追踪**：点赞、评论、收藏、分享四大核心指标实时动态监测与 Sparkline 趋势分析。
- ⏰ **自然周期智能防重**：仅需设置时间跨度与增长阈值，到期自动进入保护期，彻底杜绝重复骚扰。
- 🤖 **飞书机器人无缝推送**：支持 HMAC-SHA256 签名密钥与关键词过滤，自动关联作品负责人称谓。
- 👤 **负责人快捷管理**：常用责任人快捷点选，支持下拉面板一键删除（×）维护。
- 🛡️ **多账号沙箱隔离**：各账号独立浏览器沙箱环境，扫码一次持久有效。
- 🩺 **运行环境全项体检**：WebView2、Google Chrome、网络质量与 SQLite 磁盘健康全项诊断。
- ⚡ **本地极简与自动滚动覆盖**：7 天自动淘汰旧快照并执行 VACUUM 释放磁盘空间。
- 🪟 **系统托盘与开机自启动**：关闭窗口自动最小化托盘，10 分钟定时静默调度。

### 📦 软件包下载指南
- **\`DouyinMonitor_0.1.0_x64-setup.exe\`** (推荐)：NSIS 中文引导安装程序，适合大多数 Windows 用户直接安装运行；
- **\`DouyinMonitor_0.1.0_x64_en-US.msi\`**：标准 Windows Installer 安装程序，适合企业或需要静默安装的场景。`;

async function main() {
  console.log(`>>> 检查 Release (${TAG})...`);
  let release = null;
  const getRes = await fetch(`https://api.github.com/repos/${REPO}/releases/tags/${TAG}`, { headers });
  if (getRes.ok) {
    release = await getRes.json();
    console.log(`>>> 找到已有 Release ID: ${release.id}，更新发布说明...`);
    const updateRes = await fetch(`https://api.github.com/repos/${REPO}/releases/${release.id}`, {
      method: "PATCH",
      headers: { ...headers, "Content-Type": "application/json" },
      body: JSON.stringify({
        name: RELEASE_NAME,
        body: releaseBody,
        draft: false,
        prerelease: false
      })
    });
    release = await updateRes.json();
  } else {
    console.log(`>>> 创建全新 Release (${TAG})...`);
    const createRes = await fetch(`https://api.github.com/repos/${REPO}/releases`, {
      method: "POST",
      headers: { ...headers, "Content-Type": "application/json" },
      body: JSON.stringify({
        tag_name: TAG,
        target_commitish: "main",
        name: RELEASE_NAME,
        body: releaseBody,
        draft: false,
        prerelease: false
      })
    });
    if (!createRes.ok) {
      const errText = await createRes.text();
      throw new Error(`创建 Release 失败 (${createRes.status}): ${errText}`);
    }
    release = await createRes.json();
  }

  console.log(`>>> Release 页面: ${release.html_url}`);

  const assetsToUpload = [
    {
      filePath: "src-tauri/target/release/bundle/nsis/DouyinMonitor_0.1.0_x64-setup.exe",
      name: "DouyinMonitor_0.1.0_x64-setup.exe"
    },
    {
      filePath: "src-tauri/target/release/bundle/msi/DouyinMonitor_0.1.0_x64_en-US.msi",
      name: "DouyinMonitor_0.1.0_x64_en-US.msi"
    }
  ];

  // 查询当前已有资产
  const listAssetsRes = await fetch(`https://api.github.com/repos/${REPO}/releases/${release.id}/assets`, { headers });
  const existingAssets = listAssetsRes.ok ? await listAssetsRes.json() : [];

  for (const asset of assetsToUpload) {
    if (!fs.existsSync(asset.filePath)) {
      console.warn(`[WARN] 文件不存在: ${asset.filePath}`);
      continue;
    }

    // 检查是否存在同名旧资产，若存在先删除
    const matched = existingAssets.find((a) => a.name === asset.name);
    if (matched) {
      console.log(`>>> 删除已存在的同名资产: ${asset.name} (ID: ${matched.id})...`);
      await fetch(`https://api.github.com/repos/${REPO}/releases/assets/${matched.id}`, {
        method: "DELETE",
        headers
      });
    }

    const fileBuffer = fs.readFileSync(asset.filePath);
    const sizeMb = (fileBuffer.length / (1024 * 1024)).toFixed(2);
    const uploadUrl = `https://uploads.github.com/repos/${REPO}/releases/${release.id}/assets?name=${encodeURIComponent(asset.name)}`;
    let uploadSuccess = false;
    for (let attempt = 1; attempt <= 5; attempt++) {
      try {
        console.log(`>>> 正在上传资产 (尝试 ${attempt}/5): ${asset.name} (${sizeMb} MB)...`);
        const uploadRes = await fetch(uploadUrl, {
          method: "POST",
          headers: {
            ...headers,
            "Content-Type": "application/octet-stream",
            "Content-Length": String(fileBuffer.length)
          },
          body: fileBuffer
        });

        if (!uploadRes.ok) {
          const err = await uploadRes.text();
          console.error(`上传响应状态错误 (${uploadRes.status}): ${err}`);
          await new Promise((r) => setTimeout(r, 2000 * attempt));
          continue;
        }

        const uploaded = await uploadRes.json();
        console.log(`>>> 上传成功: ${uploaded.name} -> ${uploaded.browser_download_url}`);
        uploadSuccess = true;
        break;
      } catch (err) {
        console.warn(`第 ${attempt} 次网络异常 (${err.message})，正在重试...`);
        await new Promise((r) => setTimeout(r, 3000 * attempt));
      }
    }

    if (!uploadSuccess) {
      throw new Error(`文件 ${asset.name} 上传多次失败，请检查网络环境。`);
    }
  }

  console.log("\n==========================================");
  console.log("🎉 发布全部完成！");
  console.log(`Release 链接: ${release.html_url}`);
  console.log("==========================================");
}

main().catch((err) => {
  console.error("执行异常:", err);
  process.exit(1);
});
