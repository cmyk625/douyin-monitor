// 通用格式化与轻量计算工具

export function fmtNum(n: number | null | undefined): string {
  if (n === null || n === undefined) return "—";
  const abs = Math.abs(n);
  if (abs >= 100_000_000) return trimZero(n / 100_000_000) + "亿";
  if (abs >= 10_000) return trimZero(n / 10_000) + "万";
  return String(n);
}

function trimZero(v: number): string {
  return (Math.round(v * 10) / 10).toFixed(1).replace(/\.0$/, "");
}

export function fmtDelta(n: number | null | undefined): string {
  if (n === null || n === undefined) return "—";
  return (n > 0 ? "+" : "") + fmtNum(n);
}

/**
 * 从可能包含分享文案的长文本中自动截取抖音链接（支持短链与网页长链）
 */
export function extractDouyinUrl(text: string): string {
  if (!text) return "";
  const trimmed = text.trim();
  // 1. 匹配分享短链，如 https://v.douyin.com/gnU2yTg_HFs/
  const shortMatch = trimmed.match(/https?:\/\/v\.douyin\.com\/[a-zA-Z0-9_\-]+\/?/);
  if (shortMatch) {
    return shortMatch[0];
  }
  // 2. 匹配标准网页长链，如 https://www.douyin.com/video/1234567890 或 /note/
  const longMatch = trimmed.match(/https?:\/\/(?:www\.|creator\.)?douyin\.com\/(?:video|note)\/\d+/);
  if (longMatch) {
    return longMatch[0];
  }
  // 3. 通用 http/https 链接
  const generalMatch = trimmed.match(/https?:\/\/[^\s]+/);
  if (generalMatch) {
    return generalMatch[0];
  }
  return trimmed;
}

/**
 * 解析用户输入的规则增量阈值：
 * 支持纯数字（如 1000）、带单位（如 1万、0.5万、1w、1.5W）、千分位（如 1,000）
 */
export function parseThreshold(val: string | number | null | undefined): number | null {
  if (val === null || val === undefined) return null;
  if (typeof val === "number") {
    return Number.isFinite(val) && val >= 1 ? Math.round(val) : null;
  }
  let str = String(val).trim().replace(/[,\s，]/g, "");
  if (!str) return null;

  let factor = 1;
  if (str.endsWith("万") || str.endsWith("w") || str.endsWith("W")) {
    factor = 10_000;
    str = str.slice(0, -1).trim();
  } else if (str.endsWith("亿")) {
    factor = 100_000_000;
    str = str.slice(0, -1).trim();
  }

  const num = parseFloat(str);
  if (!Number.isFinite(num) || num <= 0) return null;
  const result = Math.round(num * factor);
  return result >= 1 ? result : null;
}

/**
 * 格式化规则增量阈值展示：
 * 满 1 万显示 "+1万 (10,000)" 或 "+0.5万 (5,000)"，不足 1 万显示 "+1,000"
 */
export function fmtThreshold(threshold: number | null | undefined): string {
  if (threshold === null || threshold === undefined) return "—";
  if (threshold >= 10_000) {
    const wan = (threshold / 10_000).toFixed(1).replace(/\.0$/, "");
    return `${wan}万 (${threshold.toLocaleString()})`;
  }
  return threshold.toLocaleString();
}

export function fmtTime(ts: number | null | undefined): string {
  if (!ts) return "—";
  const d = new Date(ts * 1000);
  const p = (v: number) => String(v).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
}

export function fmtShort(ts: number | null | undefined): string {
  if (!ts) return "—";
  const d = new Date(ts * 1000);
  const p = (v: number) => String(v).padStart(2, "0");
  const now = new Date();
  if (d.toDateString() === now.toDateString()) return `${p(d.getHours())}:${p(d.getMinutes())}`;
  return `${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
}

export function fmtPublishDate(ts: number | null | undefined): string {
  if (!ts) return "—";
  const d = new Date(ts * 1000);
  const p = (v: number) => String(v).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
}

export function fmtAgo(ts: number | null | undefined): string {
  if (!ts) return "从未";
  const diff = Math.floor(Date.now() / 1000) - ts;
  if (diff < 0) return "刚刚";
  if (diff < 60) return `${diff} 秒前`;
  if (diff < 3600) return `${Math.floor(diff / 60)} 分钟前`;
  if (diff < 86400) return `${Math.floor(diff / 3600)} 小时前`;
  return `${Math.floor(diff / 86400)} 天前`;
}

export function fmtFuture(ts: number | null | undefined): string {
  if (!ts) return "未计划";
  const diff = ts - Math.floor(Date.now() / 1000);
  if (diff <= 0) return "即将触发";
  if (diff < 60) return `${diff} 秒后`;
  if (diff < 3600) return `${Math.ceil(diff / 60)} 分钟后`;
  return `${(diff / 3600).toFixed(1)} 小时后`;
}

export function fmtDuration(secs: number | null | undefined): string {
  if (!secs || secs <= 0) return "—";
  if (secs < 60) return `${secs} 秒`;
  if (secs < 3600) return `${Math.round(secs / 60)} 分钟`;
  return `${(secs / 3600).toFixed(1)} 小时`;
}

export function fmtBytes(bytes: number | null | undefined): string {
  if (!bytes || bytes <= 0) return "0 B";
  if (bytes >= 1024 * 1024 * 1024) return (bytes / (1024 * 1024 * 1024)).toFixed(2) + " GB";
  if (bytes >= 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(2) + " MB";
  if (bytes >= 1024) return (bytes / 1024).toFixed(1) + " KB";
  return bytes + " B";
}

export const METRIC_LABELS: Record<string, string> = {
  like: "点赞",
  comment: "评论",
  share: "分享",
  collect: "收藏",
};

export function metricLabel(metric: string): string {
  return METRIC_LABELS[metric] ?? metric;
}

export function loginStateText(state: string): string {
  switch (state) {
    case "ok":
      return "已登录";
    case "expired":
      return "未登录/已失效";
    case "challenge":
      return "需要人工验证";
    case "error":
      return "采集异常";
    default:
      return "待登录";
  }
}

export function loginStateClass(state: string): string {
  switch (state) {
    case "ok":
      return "badge ok";
    case "expired":
      return "badge warn";
    case "challenge":
      return "badge danger";
    case "error":
      return "badge danger";
    default:
      return "badge muted";
  }
}

/** 生成折线 sparkline 的 SVG path，返回 { line, area, lastPoint } */
export function sparkPath(
  values: Array<number | null>,
  width: number,
  height: number,
): { line: string; area: string; lastPoint: [number, number] | null } {
  const nums = values.filter((v): v is number => v !== null && v !== undefined);
  if (nums.length === 0) return { line: "", area: "", lastPoint: null };
  const min = Math.min(...nums);
  const max = Math.max(...nums);
  const span = max - min || 1;
  const step = values.length > 1 ? width / (values.length - 1) : width;
  const pts: Array<[number, number]> = [];
  values.forEach((v, i) => {
    if (v === null || v === undefined) return;
    const x = i * step;
    const y = height - ((v - min) / span) * (height - 8) - 4;
    pts.push([x, y]);
  });
  if (pts.length === 0) return { line: "", area: "", lastPoint: null };
  const line = pts.map(([x, y], i) => `${i === 0 ? "M" : "L"}${x.toFixed(1)},${y.toFixed(1)}`).join(" ");
  const area = `${line} L${pts[pts.length - 1][0].toFixed(1)},${height} L${pts[0][0].toFixed(1)},${height} Z`;
  const lastPoint = pts[pts.length - 1];
  return { line, area, lastPoint };
}

export function pct(values: Array<number | null>, index: number): number {
  const max = Math.max(...values.filter((v): v is number => v !== null && v !== undefined), 1);
  const v = values[index];
  return v === null || v === undefined ? 0 : Math.round((v / max) * 100);
}
