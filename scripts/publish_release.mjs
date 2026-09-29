import { execSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

const tauriConf = JSON.parse(fs.readFileSync("src-tauri/tauri.conf.json", "utf-8"));
const VERSION = tauriConf.version || "0.1.1";
const TAG = `v${VERSION}`;
const RELEASE_NAME = `Douyin Monitor Desktop v${VERSION}`;
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

### 🌟 v${VERSION} 版本更新亮点
- 📅 **监控作品列表新增「发布时间」列**：支持发布时间正序/倒序动态排序，鼠标悬停可查看精确发布时间戳。
- 🎯 **操作列纯文字交互与固定贴右（Sticky Right）**：
  - 操作按钮全新升级为纯文字按钮（「采集」、「趋势」、「打开」、「编辑」、「删除」），界面更直观；
  - 表格操作列固定在最右侧并附带微阴影与分割线，横向滚动浏览复杂数据指标时操作常驻不遮挡。
- 🎨 **全局分页选择器与快捷跳页美化**：
  - 每页条数选择器升级为 shadcn UI 组件，支持自适应方向弹性弹出与勾选标记；
  - 快速跳页输入框与暗色调主题风格全面统一。
- 🛡️ **重新安装与覆盖安装优化（数据防丢失保护）**：
  - 新版本安装时自动终止后台托盘常驻进程，彻底解决“文件被占用”的覆盖安装失败问题；
  - 彻底保护系统数据目录，无论是升级、直接覆盖安装或重装，本地 SQLite 数据库、Cookie 登录会话与飞书配置永久留存、绝不丢失！
- ⚙️ **工程规范与提交工具链支持**：
  - 集成 Commitlint + cz-git 交互式提交助手，支持一键 \`pnpm commit\` 格式化提交消息。

### 📦 软件包下载指南
- **\`DouyinMonitor_${VERSION}_x64-setup.exe\`** (推荐)：NSIS 中文引导安装程序，支持新旧版本一键平滑升级与直接覆盖安装；
- **\`DouyinMonitor_${VERSION}_x64_en-US.msi\`**：标准 Windows Installer 安装程序，适合企业域或静默安装场景。`;

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
      filePath: `src-tauri/target/release/bundle/nsis/DouyinMonitor_${VERSION}_x64-setup.exe`,
      name: `DouyinMonitor_${VERSION}_x64-setup.exe`
    },
    {
      filePath: `src-tauri/target/release/bundle/msi/DouyinMonitor_${VERSION}_x64_en-US.msi`,
      name: `DouyinMonitor_${VERSION}_x64_en-US.msi`
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
