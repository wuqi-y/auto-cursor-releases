import { invoke } from "@tauri-apps/api/core";

export interface HaozhuApiConfig {
  serverBase: string;
  user: string;
  pass: string;
  token: string;
  sid: string;
  lastPhone: string;
  uid: string;
  author: string;
  isp: string;
  province: string;
  ascription: string;
  paragraph: string;
  exclude: string;
  pollIntervalSec: number;
  autoBlacklistAfterMinutes: number;
}

/** 内置开发者账号（分成）；可在 ~/.auto-cursor-vip/haozhu_api_config.json 改 `author` */
export const DEFAULT_HAOZHU_AUTHOR = "wuqi2002522";

export const DEFAULT_HAOZHU_CONFIG: HaozhuApiConfig = {
  serverBase: "https://api.haozhuma.com",
  user: "",
  pass: "",
  token: "",
  sid: "",
  lastPhone: "",
  uid: "",
  author: DEFAULT_HAOZHU_AUTHOR,
  isp: "",
  province: "",
  ascription: "",
  paragraph: "",
  exclude: "",
  pollIntervalSec: 15,
  autoBlacklistAfterMinutes: 3,
};

export function mergeHaozhuConfig(raw: unknown): HaozhuApiConfig {
  if (!raw || typeof raw !== "object") {
    return { ...DEFAULT_HAOZHU_CONFIG };
  }
  return { ...DEFAULT_HAOZHU_CONFIG, ...(raw as Partial<HaozhuApiConfig>) };
}

export async function loadHaozhuConfig(): Promise<HaozhuApiConfig> {
  const str = await invoke<string>("read_haozhu_api_config");
  if (!str.trim()) {
    return { ...DEFAULT_HAOZHU_CONFIG };
  }
  try {
    return mergeHaozhuConfig(JSON.parse(str));
  } catch {
    return { ...DEFAULT_HAOZHU_CONFIG };
  }
}

export async function saveHaozhuConfig(config: HaozhuApiConfig): Promise<void> {
  await invoke("save_haozhu_api_config", {
    config: JSON.stringify(config, null, 2),
  });
}

/** 构造 https://host/sms/?api=...&... */
export function buildSmsUrl(
  serverBase: string,
  apiName: string,
  params: Record<string, string | number | undefined | null>
): string {
  const root = serverBase.replace(/\/+$/, "");
  const url = new URL(`${root}/sms/`);
  url.searchParams.set("api", apiName);
  for (const [k, v] of Object.entries(params)) {
    if (v === undefined || v === null) continue;
    const s = String(v).trim();
    if (s === "") continue;
    url.searchParams.set(k, s);
  }
  return url.toString();
}

export async function haozhuHttpGet(fullUrl: string): Promise<string> {
  return invoke<string>("haozhu_http_get", { fullUrl });
}

/** 解析接口是否成功（文档中 code 类型不一致） */
export function parseHaozhuJson(text: string): Record<string, unknown> | null {
  try {
    const v = JSON.parse(text) as unknown;
    return typeof v === "object" && v !== null && !Array.isArray(v)
      ? (v as Record<string, unknown>)
      : null;
  } catch {
    return null;
  }
}

export function isHaozhuSuccess(code: unknown): boolean {
  if (code === undefined || code === null) return false;
  const n = typeof code === "number" ? code : Number(code);
  if (!Number.isNaN(n) && (n === 0 || n === 200)) return true;
  const s = String(code).trim();
  return s === "0" || s === "200";
}

export async function openHaozhuApiWindow(): Promise<void> {
  await invoke("open_haozhu_api_window");
}
