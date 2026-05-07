import React, { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { TitleBar } from "../components/TitleBar";
import { ToastManager, useToast } from "../components/Toast";
import { useConfigStore } from "../stores/configStore";
import { broadcastThemeMode } from "../utils/themeBroadcast";
import {
  type HaozhuApiConfig,
  buildSmsUrl,
  haozhuHttpGet,
  isHaozhuSuccess,
  loadHaozhuConfig,
  parseHaozhuJson,
  saveHaozhuConfig,
} from "../services/haozhuApiService";

/** 与 runRequest 第一个参数一致，便于按钮 loading 对照 */
const HAOZHU_BUSY = {
  login: "登录",
  summary: "账户信息",
  getPhone: "获取号码",
  designate: "指定号码",
  getMessage: "获取验证码",
  cancelRecv: "释放号码",
  cancelAll: "释放全部",
  blacklist: "拉黑号码",
} as const;

type HaozhuBusyLabel = (typeof HAOZHU_BUSY)[keyof typeof HAOZHU_BUSY];

function Spinner({ className = "h-4 w-4 shrink-0" }: { className?: string }) {
  return (
    <svg
      className={`animate-spin ${className}`}
      viewBox="0 0 24 24"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      aria-hidden
    >
      <circle
        className="opacity-25"
        cx="12"
        cy="12"
        r="10"
        stroke="currentColor"
        strokeWidth="4"
      />
      <path
        className="opacity-75"
        fill="currentColor"
        d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
      />
    </svg>
  );
}

function ActionButton({
  busyLabel,
  busy,
  loadingText,
  className,
  disabled,
  onClick,
  children,
}: {
  busyLabel: HaozhuBusyLabel;
  busy: string;
  loadingText: string;
  className: string;
  disabled?: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  const loading = busy === busyLabel;
  return (
    <button
      type="button"
      disabled={disabled || !!busy}
      aria-busy={loading}
      className={`inline-flex min-h-[2.5rem] items-center justify-center gap-2 ${className}`}
      onClick={onClick}
    >
      {loading && <Spinner />}
      {loading ? loadingText : children}
    </button>
  );
}

function Field({
  label,
  hint,
  ...inputProps
}: React.InputHTMLAttributes<HTMLInputElement> & {
  label: string;
  hint?: string;
}) {
  return (
    <label className="block space-y-1">
      <span className="text-xs font-medium text-slate-600 dark:text-slate-400">
        {label}
      </span>
      {hint && (
        <p className="text-[11px] text-slate-500 dark:text-slate-500">{hint}</p>
      )}
      <input
        className="w-full px-3 py-2 text-sm bg-white border shadow-sm outline-none rounded-xl border-slate-200 text-slate-900 ring-blue-500/30 focus:border-blue-500 focus:ring-2 dark:border-slate-700 dark:bg-slate-900 dark:text-slate-100"
        {...inputProps}
      />
    </label>
  );
}

export const HaozhuApiPage: React.FC = () => {
  const themeMode = useConfigStore((state) => state.getThemeMode() ?? "light");
  const setThemeMode = useConfigStore((state) => state.setThemeMode);
  const { toasts, removeToast, showError, showWarning } = useToast();
  const toastRef = useRef({ showError, showWarning });
  toastRef.current = { showError, showWarning };

  const [cfg, setCfg] = useState<HaozhuApiConfig | null>(null);
  const [lastRaw, setLastRaw] = useState<string>("");
  const [busy, setBusy] = useState<string>("");
  const [polling, setPolling] = useState(false);
  /** 轮询单次请求进行中（用于按钮 loading） */
  const [pollTickBusy, setPollTickBusy] = useState(false);
  /** getSummary 成功后展示在面板（余额等） */
  const [accountPanel, setAccountPanel] = useState<{
    money: string;
    num: string;
    lx?: string;
    fanliJe?: string;
  } | null>(null);
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const phoneSinceRef = useRef<number | null>(null);
  const warnedTimeoutRef = useRef(false);
  const cfgRef = useRef(cfg);
  cfgRef.current = cfg;

  useEffect(() => {
    void loadHaozhuConfig().then(setCfg);
  }, []);

  const persist = useCallback(async (next: HaozhuApiConfig) => {
    try {
      await saveHaozhuConfig(next);
    } catch (e) {
      toastRef.current.showError(
        e instanceof Error ? e.message : String(e),
      );
    }
  }, []);

  useEffect(() => {
    if (!cfg) return;
    if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
    saveTimerRef.current = setTimeout(() => {
      void persist(cfg);
    }, 450);
    return () => {
      if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
    };
  }, [cfg, persist]);

  const updateCfg = useCallback((patch: Partial<HaozhuApiConfig>) => {
    setCfg((c) => (c ? { ...c, ...patch } : c));
  }, []);

  const runRequest = useCallback(
    async (
      label: string,
      url: string,
      onSuccess?: (parsed: Record<string, unknown>) => void,
    ) => {
      setBusy(label);
      try {
        const raw = await haozhuHttpGet(url);
        setLastRaw(raw);
        const parsed = parseHaozhuJson(raw);
        if (parsed && onSuccess) onSuccess(parsed);
        return raw;
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        toastRef.current.showError(msg);
        throw e;
      } finally {
        setBusy("");
      }
    },
    [],
  );

  const handleLogin = async () => {
    if (!cfg) return;
    await runRequest(
      HAOZHU_BUSY.login,
      buildSmsUrl(cfg.serverBase, "login", {
        user: cfg.user,
        pass: cfg.pass,
      }),
      (p) => {
        if (isHaozhuSuccess(p.code)) {
          const tok = p.token;
          if (typeof tok === "string" && tok.length > 0) {
            updateCfg({ token: tok });
          }
        }
      },
    );
  };

  const handleSummary = async () => {
    if (!cfg?.token) {
      showWarning("请先登录获取 token");
      return;
    }
    await runRequest(
      HAOZHU_BUSY.summary,
      buildSmsUrl(cfg.serverBase, "getSummary", { token: cfg.token }),
      (p) => {
        if (!isHaozhuSuccess(p.code)) return;
        const money =
          p.money !== undefined && p.money !== null ? String(p.money) : "";
        const num =
          p.num !== undefined && p.num !== null ? String(p.num) : "";
        const lx =
          p.lx !== undefined && p.lx !== null ? String(p.lx) : undefined;
        const fanliJe =
          p.fanli_je !== undefined && p.fanli_je !== null
            ? String(p.fanli_je)
            : undefined;
        setAccountPanel({
          money,
          num,
          lx,
          fanliJe,
        });
      },
    );
  };

  const commonPhoneParams = (c: HaozhuApiConfig) => ({
    token: c.token,
    sid: c.sid,
    isp: c.isp || undefined,
    Province: c.province || undefined,
    ascription: c.ascription || undefined,
    paragraph: c.paragraph || undefined,
    exclude: c.exclude || undefined,
    uid: c.uid || undefined,
    author: c.author || undefined,
  });

  const handleGetPhone = async () => {
    if (!cfg?.token || !cfg.sid) {
      showWarning("需要 token 与项目 sid");
      return;
    }
    await runRequest(
      HAOZHU_BUSY.getPhone,
      buildSmsUrl(cfg.serverBase, "getPhone", commonPhoneParams(cfg)),
      (p) => {
        if (isHaozhuSuccess(p.code)) {
          const ph = p.phone;
          if (typeof ph === "string" || typeof ph === "number") {
            updateCfg({ lastPhone: String(ph) });
          }
          phoneSinceRef.current = Date.now();
          warnedTimeoutRef.current = false;
        }
      },
    );
  };

  const handleDesignatePhone = async () => {
    if (!cfg?.token || !cfg.sid || !cfg.lastPhone) {
      showWarning("需要 token、sid 与号码（上次号码或手动填写）");
      return;
    }
    await runRequest(
      HAOZHU_BUSY.designate,
      buildSmsUrl(cfg.serverBase, "getPhone", {
        ...commonPhoneParams(cfg),
        phone: cfg.lastPhone,
      }),
      (p) => {
        if (isHaozhuSuccess(p.code)) {
          phoneSinceRef.current = Date.now();
          warnedTimeoutRef.current = false;
        }
      },
    );
  };

  const handleGetMessage = async () => {
    if (!cfg?.token || !cfg.sid || !cfg.lastPhone) {
      showWarning("需要 token、sid 与号码");
      return;
    }
    await runRequest(
      HAOZHU_BUSY.getMessage,
      buildSmsUrl(cfg.serverBase, "getMessage", {
        token: cfg.token,
        sid: cfg.sid,
        phone: cfg.lastPhone,
      }),
      () => {},
    );
  };

  const stopPoll = useCallback(() => {
    if (pollRef.current) {
      clearInterval(pollRef.current);
      pollRef.current = null;
    }
    setPolling(false);
  }, []);

  useEffect(() => {
    return () => stopPoll();
  }, [stopPoll]);

  const startPoll = () => {
    const c = cfgRef.current;
    if (!c) return;
    stopPoll();
    const sec = Math.max(5, c.pollIntervalSec || 15);
    setPolling(true);
    const tick = () => {
      void (async () => {
        const cfgTick = cfgRef.current;
        if (!cfgTick?.token || !cfgTick.sid || !cfgTick.lastPhone) return;
        setPollTickBusy(true);
        try {
          const url = buildSmsUrl(cfgTick.serverBase, "getMessage", {
            token: cfgTick.token,
            sid: cfgTick.sid,
            phone: cfgTick.lastPhone,
          });
          const raw = await haozhuHttpGet(url);
          setLastRaw(raw);
          const p = parseHaozhuJson(raw);
          if (p && isHaozhuSuccess(p.code)) {
            const yzm = p.yzm;
            if (yzm !== undefined && yzm !== null && String(yzm).length > 0) {
              stopPoll();
            }
          }
          const limitMin = cfgTick.autoBlacklistAfterMinutes ?? 3;
          if (
            phoneSinceRef.current &&
            Date.now() - phoneSinceRef.current > limitMin * 60_000 &&
            !warnedTimeoutRef.current
          ) {
            warnedTimeoutRef.current = true;
            toastRef.current.showWarning(
              `已超过 ${limitMin} 分钟仍未收到验证码，号码可能欠费。可考虑调用「拉黑号码」。`,
            );
          }
        } catch (e) {
          toastRef.current.showError(
            e instanceof Error ? e.message : String(e),
          );
        } finally {
          setPollTickBusy(false);
        }
      })();
    };
    tick();
    pollRef.current = setInterval(tick, sec * 1000);
  };

  const handleCancelRecv = async () => {
    if (!cfg?.token || !cfg.sid || !cfg.lastPhone) return;
    await runRequest(
      HAOZHU_BUSY.cancelRecv,
      buildSmsUrl(cfg.serverBase, "cancelRecv", {
        token: cfg.token,
        sid: cfg.sid,
        phone: cfg.lastPhone,
      }),
    );
  };

  const handleCancelAll = async () => {
    if (!cfg?.token) return;
    await runRequest(
      HAOZHU_BUSY.cancelAll,
      buildSmsUrl(cfg.serverBase, "cancelAllRecv", { token: cfg.token }),
    );
  };

  const handleBlacklist = async () => {
    if (!cfg?.token || !cfg.sid || !cfg.lastPhone) return;
    await runRequest(
      HAOZHU_BUSY.blacklist,
      buildSmsUrl(cfg.serverBase, "addBlacklist", {
        token: cfg.token,
        sid: cfg.sid,
        phone: cfg.lastPhone,
      }),
    );
  };

  const openExternal = async (url: string) => {
    try {
      await invoke("open_update_url", { url });
    } catch (e) {
      toastRef.current.showError(e instanceof Error ? e.message : String(e));
    }
  };

  if (!cfg) {
    return (
      <div className="flex items-center justify-center h-screen app-shell window-frame">
        <p className="text-sm text-slate-500">加载配置…</p>
      </div>
    );
  }

  const parsed = parseHaozhuJson(lastRaw);
  const btn =
    "rounded-xl px-4 py-2 text-sm font-medium transition-colors disabled:pointer-events-none disabled:opacity-55";
  const btnPrimary = `${btn} bg-blue-600 text-white hover:bg-blue-700`;
  const btnMuted = `${btn} border border-slate-200 bg-slate-50 text-slate-800 hover:bg-slate-100 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-100`;
  const btnDanger = `${btn} border border-red-200 bg-red-50 text-red-800 hover:bg-red-100 dark:border-red-900 dark:bg-red-950 dark:text-red-200`;

  return (
    <div className="flex flex-col h-screen min-h-0 app-shell window-frame">
      <div className="flex flex-col flex-1 min-h-0 window-surface">
        <TitleBar />
        <main className="flex-1 p-4 overflow-auto scrollbar-none sm:p-6">
          <div className="max-w-4xl pb-16 mx-auto space-y-8">
            <header className="space-y-2">
              <h1 className="text-xl font-bold text-slate-900 dark:text-slate-100">
                豪猪 API
              </h1>
              <p className="text-sm text-slate-600 dark:text-slate-400">
                配置保存在{" "}
                <code className="px-1 rounded bg-slate-100 dark:bg-slate-800">
                  ~/.auto-cursor-vip/haozhu_api_config.json
                </code>
                。请先登录获取 token，勿每次取号都登录。
              </p>
              <div className="flex flex-wrap gap-2 text-xs">
                <button
                  type="button"
                  className="text-blue-600 underline dark:text-blue-400"
                  onClick={() => void openExternal("https://h5.haozhuyun.com")}
                >
                  H5 控制台（豪猪云）
                </button>
                <button
                  type="button"
                  className="text-blue-600 underline dark:text-blue-400"
                  onClick={() => void openExternal("https://h5.haozhuma.com")}
                >
                  H5 控制台（豪猪码）
                </button>
                <button
                  type="button"
                  onClick={() => {
                    const next = themeMode === "dark" ? "light" : "dark";
                    setThemeMode(next);
                    void broadcastThemeMode(next);
                  }}
                  className="rounded-lg border border-slate-200 px-2 py-0.5 dark:border-slate-600"
                >
                  {themeMode === "dark" ? "暗色" : "浅色"}
                </button>
              </div>
            </header>

            {accountPanel && (
              <section
                className="flex flex-wrap items-end gap-6 border border-emerald-200/80 bg-emerald-50/90 p-4 shadow-sm rounded-2xl dark:border-emerald-900/50 dark:bg-emerald-950/40"
                aria-live="polite"
              >
                <div>
                  <div className="text-xs font-medium text-emerald-800/80 dark:text-emerald-300/90">
                    账户余额（getSummary · money）
                  </div>
                  <div className="mt-1 text-3xl font-bold tabular-nums tracking-tight text-emerald-700 dark:text-emerald-400">
                    {accountPanel.money !== ""
                      ? `¥ ${accountPanel.money}`
                      : "—"}
                  </div>
                </div>
                {accountPanel.num !== "" && (
                  <div>
                    <div className="text-xs font-medium text-slate-600 dark:text-slate-400">
                      最大区号数量 num
                    </div>
                    <div className="mt-1 text-lg font-semibold tabular-nums text-slate-900 dark:text-slate-100">
                      {accountPanel.num}
                    </div>
                  </div>
                )}
                {accountPanel.lx && (
                  <div>
                    <div className="text-xs font-medium text-slate-600 dark:text-slate-400">
                      类型 lx
                    </div>
                    <div className="mt-1 text-sm text-slate-800 dark:text-slate-200">
                      {accountPanel.lx}
                    </div>
                  </div>
                )}
                {accountPanel.fanliJe !== undefined &&
                  accountPanel.fanliJe !== "" && (
                    <div>
                      <div className="text-xs font-medium text-slate-600 dark:text-slate-400">
                        返利金额 fanli_je
                      </div>
                      <div className="mt-1 text-sm font-medium tabular-nums text-slate-800 dark:text-slate-200">
                        ¥ {accountPanel.fanliJe}
                      </div>
                    </div>
                  )}
              </section>
            )}

            <section className="p-5 space-y-4 border shadow-sm rounded-2xl border-slate-200/80 bg-white/90 dark:border-slate-800 dark:bg-slate-950/80">
              <h2 className="text-sm font-semibold text-slate-900 dark:text-slate-100">
                连接与凭证
              </h2>
              <div className="grid gap-4 sm:grid-cols-2">
                <Field
                  label="服务器地址（完整 HTTPS Origin）"
                  hint="如 https://api.haozhuma.com 或 https://api.haozhuyun.com"
                  value={cfg.serverBase}
                  onChange={(e) => updateCfg({ serverBase: e.target.value })}
                />
                <Field
                  label="API 账号 user"
                  value={cfg.user}
                  onChange={(e) => updateCfg({ user: e.target.value })}
                />
                <Field
                  label="API 密码 pass"
                  type="password"
                  value={cfg.pass}
                  onChange={(e) => updateCfg({ pass: e.target.value })}
                />
                <Field
                  label="Token（登录后自动写入）"
                  value={cfg.token}
                  onChange={(e) => updateCfg({ token: e.target.value })}
                />
              </div>
              <div className="flex flex-wrap gap-2">
                <ActionButton
                  busyLabel={HAOZHU_BUSY.login}
                  busy={busy}
                  loadingText="登录中…"
                  className={btnPrimary}
                  onClick={() => void handleLogin()}
                >
                  登录获取 token
                </ActionButton>
                <ActionButton
                  busyLabel={HAOZHU_BUSY.summary}
                  busy={busy}
                  loadingText="查询中…"
                  className={btnMuted}
                  disabled={!cfg.token}
                  onClick={() => void handleSummary()}
                >
                  获取账户信息 getSummary
                </ActionButton>
              </div>
            </section>

            <section className="p-5 space-y-4 border shadow-sm rounded-2xl border-slate-200/80 bg-white/90 dark:border-slate-800 dark:bg-slate-950/80">
              <h2 className="text-sm font-semibold text-slate-900 dark:text-slate-100">
                项目与筛选参数（getPhone）
              </h2>
              {/* <p className="text-[11px] text-slate-500 dark:text-slate-400">
                开发者 author 已内置为{" "}
                <code className="px-1 rounded bg-slate-100 dark:bg-slate-800">
                  wuqi2002522
                </code>
                ，不在此填写；需修改或清空请在{" "}
                <code className="px-1 rounded bg-slate-100 dark:bg-slate-800">haozhu_api_config.json</code>{" "}
                中编辑 <code className="px-1 rounded bg-slate-100 dark:bg-slate-800">author</code>。
              </p> */}
              <div className="grid gap-4 sm:grid-cols-2">
                <Field
                  label="项目 sid"
                  value={cfg.sid}
                  onChange={(e) => updateCfg({ sid: e.target.value })}
                />
                <Field
                  label="对接码 uid（可选）"
                  value={cfg.uid}
                  onChange={(e) => updateCfg({ uid: e.target.value })}
                />
                <Field
                  label="运营商 isp（可选）"
                  hint="如 1 移动 / 5 联通 / 9 电信"
                  value={cfg.isp}
                  onChange={(e) => updateCfg({ isp: e.target.value })}
                />
                <Field
                  label="省份 Province（可选）"
                  hint="如 44 广东"
                  value={cfg.province}
                  onChange={(e) => updateCfg({ province: e.target.value })}
                />
                <Field
                  label="号码类型 ascription（可选）"
                  hint="1 虚拟 / 2 实卡"
                  value={cfg.ascription}
                  onChange={(e) => updateCfg({ ascription: e.target.value })}
                />
                <Field
                  label="只取号段 paragraph（可选）"
                  hint="多段用 | 连接"
                  value={cfg.paragraph}
                  onChange={(e) => updateCfg({ paragraph: e.target.value })}
                />
                <Field
                  label="排除号段 exclude（可选）"
                  hint="多段用 | 连接"
                  value={cfg.exclude}
                  onChange={(e) => updateCfg({ exclude: e.target.value })}
                />
              </div>
              <div className="grid gap-4 sm:grid-cols-2">
                <Field
                  label="当前号码 phone（用于验证码 / 指定 / 释放 / 拉黑）"
                  value={cfg.lastPhone}
                  onChange={(e) => updateCfg({ lastPhone: e.target.value })}
                />
                <Field
                  label="轮询间隔（秒）"
                  type="number"
                  min={5}
                  value={cfg.pollIntervalSec}
                  onChange={(e) =>
                    updateCfg({ pollIntervalSec: Number(e.target.value) || 15 })
                  }
                />
                <Field
                  label="超时提示（分钟，未收到验证码）"
                  type="number"
                  min={1}
                  value={cfg.autoBlacklistAfterMinutes}
                  onChange={(e) =>
                    updateCfg({
                      autoBlacklistAfterMinutes: Number(e.target.value) || 3,
                    })
                  }
                />
              </div>
              <div className="flex flex-wrap gap-2">
                <ActionButton
                  busyLabel={HAOZHU_BUSY.getPhone}
                  busy={busy}
                  loadingText="取号中…"
                  className={btnPrimary}
                  onClick={() => void handleGetPhone()}
                >
                  获取号码
                </ActionButton>
                <ActionButton
                  busyLabel={HAOZHU_BUSY.designate}
                  busy={busy}
                  loadingText="占用中…"
                  className={btnMuted}
                  onClick={() => void handleDesignatePhone()}
                >
                  指定号码（再次接码）
                </ActionButton>
              </div>
            </section>

            <section className="p-5 space-y-4 border shadow-sm rounded-2xl border-slate-200/80 bg-white/90 dark:border-slate-800 dark:bg-slate-950/80">
              <h2 className="text-sm font-semibold text-slate-900 dark:text-slate-100">
                验证码 getMessage
              </h2>
              <div className="flex flex-wrap gap-2">
                <ActionButton
                  busyLabel={HAOZHU_BUSY.getMessage}
                  busy={busy}
                  loadingText="查询中…"
                  className={btnPrimary}
                  onClick={() => void handleGetMessage()}
                >
                  读取一次验证码
                </ActionButton>
                {!polling ? (
                  <button
                    type="button"
                    disabled={!!busy}
                    className={`inline-flex min-h-[2.5rem] items-center justify-center gap-2 ${btnMuted}`}
                    onClick={() => startPoll()}
                  >
                    开始轮询（每 {cfg.pollIntervalSec}s）
                  </button>
                ) : (
                  <button
                    type="button"
                    className={`inline-flex min-h-[2.5rem] items-center justify-center gap-2 ${btnMuted}`}
                    onClick={stopPoll}
                    aria-busy={pollTickBusy}
                  >
                    {pollTickBusy ? (
                      <>
                        <Spinner />
                        查询中…
                      </>
                    ) : (
                      "停止轮询"
                    )}
                  </button>
                )}
              </div>
              {parsed && (
                <div className="px-4 py-3 text-sm rounded-xl bg-slate-50 dark:bg-slate-900/80">
                  <div className="font-medium text-slate-800 dark:text-slate-100">
                    yzm:{" "}
                    <span className="text-blue-600 dark:text-blue-400">
                      {String(parsed.yzm ?? "—")}
                    </span>
                  </div>
                  {parsed.sms != null && (
                    <p className="mt-2 text-xs whitespace-pre-wrap text-slate-600 dark:text-slate-400">
                      {String(parsed.sms)}
                    </p>
                  )}
                </div>
              )}
            </section>

            <section className="p-5 space-y-4 border shadow-sm rounded-2xl border-slate-200/80 bg-white/90 dark:border-slate-800 dark:bg-slate-950/80">
              <h2 className="text-sm font-semibold text-slate-900 dark:text-slate-100">
                释放与拉黑
              </h2>
              <div className="flex flex-wrap gap-2">
                <ActionButton
                  busyLabel={HAOZHU_BUSY.cancelRecv}
                  busy={busy}
                  loadingText="释放中…"
                  className={btnMuted}
                  onClick={() => void handleCancelRecv()}
                >
                  释放指定号码 cancelRecv
                </ActionButton>
                <ActionButton
                  busyLabel={HAOZHU_BUSY.cancelAll}
                  busy={busy}
                  loadingText="释放中…"
                  className={btnMuted}
                  onClick={() => void handleCancelAll()}
                >
                  释放全部 cancelAllRecv
                </ActionButton>
                <ActionButton
                  busyLabel={HAOZHU_BUSY.blacklist}
                  busy={busy}
                  loadingText="处理中…"
                  className={btnDanger}
                  onClick={() => void handleBlacklist()}
                >
                  拉黑号码 addBlacklist
                </ActionButton>
              </div>
            </section>

            <section className="space-y-2">
              <h2
                id="haozhu-raw-response-heading"
                className="text-sm font-semibold text-slate-900 dark:text-slate-100"
              >
                原始响应
              </h2>
              <textarea
                readOnly
                aria-labelledby="haozhu-raw-response-heading"
                className="w-full h-48 p-3 font-mono text-xs border resize-y rounded-xl border-slate-200 bg-slate-50 text-slate-800 dark:border-slate-700 dark:bg-slate-900 dark:text-slate-200"
                value={lastRaw || "（尚无请求）"}
              />
            </section>

            <section className="p-4 text-xs border rounded-xl border-slate-200 bg-slate-50 text-slate-600 dark:border-slate-700 dark:bg-slate-900 dark:text-slate-400">
              <p className="font-medium text-slate-800 dark:text-slate-200">
                运营商 isp 参考
              </p>
              <p className="mt-1">
                1 移动 · 5 联通 · 9 电信 · 14 广电 · 16 虚拟运营商
              </p>
              <p className="mt-2 font-medium text-slate-800 dark:text-slate-200">
                省份 Province
              </p>
              <p className="mt-1">
                北京11 · 广东44 · 上海31 · 浙江33 …（完整见文档）
              </p>
            </section>
          </div>
        </main>
      </div>
      <ToastManager toasts={toasts} removeToast={removeToast} />
    </div>
  );
};
