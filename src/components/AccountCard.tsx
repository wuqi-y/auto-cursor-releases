import React, { useState, useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import { AccountInfo } from "../types/account";

interface AccountCardProps {
  account: AccountInfo;
  index: number;
  isSelected: boolean;
  actualCurrentToken: string | null;
  updateAccessTokenLoading: string | null;
  manualBindCardLoading: string | null;
  cancelSubscriptionLoading: string | null;
  openMenuEmail: string | null;
  onSelectAccount: (email: string, checked: boolean) => void;
  onSwitchAccount: (email: string) => void;
  onRemoveAccount: (email: string) => void;
  onEditAccount: (account: AccountInfo) => void;
  onViewUsage: (account: AccountInfo) => void;
  onUpdateAccessToken: (account: AccountInfo) => void;
  onReLoginAccount: (account: AccountInfo) => void;
  onViewDashboard: (account: AccountInfo) => void;
  onManualBindCard: (account: AccountInfo) => void;
  onCopyBindCardUrl: (account: AccountInfo) => void;
  onCancelSubscription: (account: AccountInfo) => void;
  onDeleteCursorAccount: (account: AccountInfo) => void;
  onSetOpenMenuEmail: (email: string | null) => void;
  onUpdateAccountTags?: (
    email: string,
    tags: Array<{ text: string; color: string }>
  ) => void;
  formatDate: (dateString: string) => string;
}

export const AccountCard: React.FC<AccountCardProps> = ({
  account,
  index,
  isSelected,
  actualCurrentToken,
  updateAccessTokenLoading,
  manualBindCardLoading,
  cancelSubscriptionLoading,
  openMenuEmail,
  onSelectAccount,
  onSwitchAccount,
  onRemoveAccount,
  onEditAccount,
  onViewUsage,
  onUpdateAccessToken,
  onReLoginAccount,
  onViewDashboard,
  onManualBindCard,
  onCopyBindCardUrl,
  onCancelSubscription,
  onDeleteCursorAccount,
  onSetOpenMenuEmail,
  onUpdateAccountTags,
  formatDate,
}) => {
  const uniqueKey = `${account.email}-${index}`;
  const dropdownRef = useRef<HTMLDivElement>(null);
  const MENU_WIDTH = 192;
  const MENU_HEIGHT = 360;
  const VIEWPORT_PADDING = 12;

  // 用于进度条动画的状态
  const [displayProgress, setDisplayProgress] = useState(0);
  const [shouldAnimate, setShouldAnimate] = useState(false);

  // 右键菜单位置状态
  const [contextMenuPos, setContextMenuPos] = useState<{
    x: number;
    y: number;
    isRightClick?: boolean; // 标记是否是右键触发
  } | null>(null);

  // 自定义标签编辑状态
  const [isEditingTags, setIsEditingTags] = useState(false);
  const [editingTags, setEditingTags] = useState<
    Array<{ text: string; color: string }>
  >(account.custom_tags || []);

  // 监听点击外部关闭下拉菜单
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (
        dropdownRef.current &&
        !dropdownRef.current.contains(event.target as Node) &&
        openMenuEmail === account.email
      ) {
        onSetOpenMenuEmail(null);
        setContextMenuPos(null); // 清除右键位置
      }
    };

    if (openMenuEmail === account.email) {
      document.addEventListener("mousedown", handleClickOutside);
    } else {
      setContextMenuPos(null); // 菜单关闭时清除位置
    }

    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
    };
  }, [openMenuEmail, account.email, onSetOpenMenuEmail]);

  const getClampedMenuPosition = (x: number, y: number) => {
    const maxX = Math.max(VIEWPORT_PADDING, window.innerWidth - MENU_WIDTH - VIEWPORT_PADDING);
    const maxY = Math.max(VIEWPORT_PADDING, window.innerHeight - MENU_HEIGHT - VIEWPORT_PADDING);

    return {
      x: Math.min(Math.max(x, VIEWPORT_PADDING), maxX),
      y: Math.min(Math.max(y, VIEWPORT_PADDING), maxY),
    };
  };

  // 监听 usage_progress 变化，触发动画
  useEffect(() => {
    if (account.usage_progress && account.usage_progress.percentage > 0) {
      // 如果是首次显示，先设置为 0 并启用动画
      if (displayProgress === 0) {
        setShouldAnimate(false);
        setDisplayProgress(0);

        // 使用单次 requestAnimationFrame 即可
        requestAnimationFrame(() => {
          setShouldAnimate(true);
          setDisplayProgress(account.usage_progress!.percentage);

          // 动画结束后移除 willChange，释放资源
          setTimeout(() => {
            setShouldAnimate(false);
          }, 1000); // 与动画时长一致
        });
      } else {
        // 如果是更新，直接更新值（已经有动画类）
        setDisplayProgress(account.usage_progress.percentage);
      }
    }
  }, [account.usage_progress?.percentage]);

  return (
    <div
      key={uniqueKey}
      onClick={() => onSelectAccount(account.email, !isSelected)}
      onContextMenu={(e) => {
        e.preventDefault();
        const position = getClampedMenuPosition(e.clientX, e.clientY);
        setContextMenuPos({
          x: position.x,
          y: position.y,
          isRightClick: true,
        });
        onSetOpenMenuEmail(account.email);
      }}
      className={`relative flex cursor-pointer flex-col rounded-lg border-2 p-3 transition-all ${
        isSelected
          ? "border-blue-500 bg-blue-50 shadow-sm dark:border-blue-500/50 dark:bg-blue-500/20"
          : actualCurrentToken && account.token == actualCurrentToken
          ? "border-emerald-300 bg-emerald-50 hover:border-emerald-400 dark:border-emerald-500/40 dark:bg-emerald-500/20 dark:hover:border-emerald-400/60"
          : "surface-elevated border-slate-200 hover:border-slate-300 hover:shadow-sm dark:border-slate-700 dark:hover:border-slate-500 dark:bg-slate-900/70 dark:hover:bg-slate-800"
      }`}
    >
      {/* 右上角标签 - 账户类型和剩余天数 */}
      <div className="absolute flex flex-col items-end gap-1 top-2 right-2">
        {/* 订阅类型角标 */}
        {account.auth_status === "authorized" && account.subscription_type && (
          <span
            className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-medium ${
              account.subscription_type.toLowerCase().includes("pro") ||
              account.subscription_type.toLowerCase().includes("business")
                ? "bg-purple-100 text-purple-800 dark:bg-purple-500/15 dark:text-purple-200"
                : account.subscription_type.toLowerCase().includes("trial")
                ? "bg-yellow-100 text-yellow-800 dark:bg-yellow-500/15 dark:text-yellow-200"
                : "bg-slate-100 text-slate-800 dark:bg-slate-800 dark:text-slate-200"
            }`}
          >
            {account.subscription_type}
          </span>
        )}
        {/* 试用剩余天数角标 */}
        {account.auth_status === "authorized" &&
          account.trial_days_remaining !== undefined &&
          account.trial_days_remaining !== null && (
            <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-orange-100 text-orange-800 dark:bg-orange-500/15 dark:text-orange-200">
              {account.trial_days_remaining} 天
            </span>
          )}
        {/* 自定义标签（只显示第一个） */}
        {account.custom_tags &&
          account.custom_tags.length > 0 &&
          account.custom_tags[0].text && (
            <span
              className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium"
              style={{
                backgroundColor: account.custom_tags[0].color + "20",
                color: account.custom_tags[0].color,
                border: `1px solid ${account.custom_tags[0].color}40`,
              }}
            >
              {account.custom_tags[0].text}
            </span>
          )}
      </div>

      <div className="flex items-start justify-between flex-1">
        <div className="flex items-start flex-1 pr-20 space-x-3">
          {/* 复选框 */}
          <input
            type="checkbox"
            checked={isSelected}
            onChange={(e) => {
              e.stopPropagation();
              onSelectAccount(account.email, e.target.checked);
            }}
            onClick={(e) => e.stopPropagation()}
            className="mt-1 h-4 w-4 rounded border-slate-300 text-blue-600 focus:ring-blue-500 dark:border-slate-600 dark:bg-slate-900"
            aria-label={`选择账户 ${account.email}`}
          />
          <div className="flex-1 min-w-0">
            {/* 邮箱单独一行 */}
            <div className="mb-2">
              <span
                className="block text-sm font-medium text-slate-900 dark:text-slate-100 truncate"
                title={account.email}
              >
                {account.email}
              </span>
            </div>
            {/* 标签行 */}
            <div className="flex flex-wrap items-center gap-1.5 mb-2">
              {/* 自动轮换状态标签 */}
              {account.isAutoSwitch === true && (
                <span className="inline-flex items-center rounded px-2 py-0.5 text-xs font-medium bg-purple-100 text-purple-800 dark:bg-purple-500/15 dark:text-purple-200">
                  🔄 轮换
                </span>
              )}
              {/* 状态标签 */}
              {account.auth_status === undefined ||
              account.subscription_type === undefined ? (
                // Loading 状态
                <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-slate-100 text-slate-500 dark:bg-slate-800 dark:text-slate-400">
                  <svg
                    className="animate-spin -ml-0.5 mr-1 h-3 w-3 text-slate-500 dark:text-slate-400"
                    xmlns="http://www.w3.org/2000/svg"
                    fill="none"
                    viewBox="0 0 24 24"
                  >
                    <circle
                      className="opacity-25"
                      cx="12"
                      cy="12"
                      r="10"
                      stroke="currentColor"
                      strokeWidth="4"
                    ></circle>
                    <path
                      className="opacity-75"
                      fill="currentColor"
                      d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
                    ></path>
                  </svg>
                  加载中
                </span>
              ) : account.auth_status === "not_fetched" ? (
                <span className="inline-flex items-center rounded px-2 py-0.5 text-xs font-medium bg-blue-100 text-blue-800 dark:bg-blue-500/15 dark:text-blue-200">
                  未获取
                </span>
              ) : account.auth_status === "unauthorized" ? (
                <span className="inline-flex items-center rounded px-2 py-0.5 text-xs font-medium bg-red-100 text-red-800 dark:bg-red-500/15 dark:text-red-200">
                  未授权
                </span>
              ) : account.auth_status === "error" ? (
                <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-orange-100 text-orange-800 dark:bg-orange-500/15 dark:text-orange-200">
                  获取失败
                </span>
              ) : account.auth_status === "network_error" ? (
                <span className="inline-flex items-center rounded px-2 py-0.5 text-xs font-medium bg-slate-100 text-slate-800 dark:bg-slate-800 dark:text-slate-200">
                  网络错误
                </span>
              ) : null}
              {/* 当前账户标签 */}
              {actualCurrentToken && account.token === actualCurrentToken && (
                <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-green-100 text-green-800 dark:bg-green-500/15 dark:text-green-200">
                  当前账户
                </span>
              )}
            </div>
            <p className="text-xs text-slate-500 dark:text-slate-400">
              {formatDate(account.created_at)}
            </p>
            {/* 错误信息显示 */}
            {account.auth_error && (
              <p className="mt-1 text-xs text-red-600">
                错误: {account.auth_error}
              </p>
            )}
            {/* 订阅状态 - 简化显示 */}
            {account.subscription_status &&
              account.auth_status === "authorized" && (
                <p className="text-xs text-slate-500 dark:text-slate-400">
                  <span
                    className={
                      account.subscription_status.toLowerCase() === "active"
                        ? "text-green-600"
                        : account.subscription_status.toLowerCase() ===
                          "trialing"
                        ? "text-yellow-600"
                        : "text-slate-500 dark:text-slate-400"
                    }
                  >
                    {account.subscription_status}
                  </span>
                </p>
              )}
          </div>
        </div>
      </div>

      {/* 操作按钮区域 - 单独一行 */}
      <div className="mt-3 flex items-center justify-between border-t border-slate-200 pt-3 dark:border-slate-700">
        <div className="flex items-center gap-2">
          {/* 常用操作：切换和删除（仅非当前账户） */}
          {!(actualCurrentToken && account.token === actualCurrentToken) && (
            <>
              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation();
                  onSwitchAccount(account.email);
                }}
                className="inline-flex items-center px-4 py-2 text-xs font-medium text-blue-700 transition-colors bg-blue-100 border border-transparent rounded-full hover:bg-blue-200 focus:outline-none focus:ring-2 focus:ring-blue-400"
              >
                切换
              </button>
              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation();
                  onRemoveAccount(account.email);
                }}
                className="inline-flex items-center px-4 py-2 text-xs font-medium text-red-700 transition-colors bg-red-100 border border-transparent rounded-full hover:bg-red-200 focus:outline-none focus:ring-2 focus:ring-red-400"
              >
                删除
              </button>
            </>
          )}
        </div>

        {/* 更多操作下拉菜单 */}
        <div ref={dropdownRef} className="relative dropdown-menu">
          <button
            type="button"
            onClick={(e) => {
              e.preventDefault();
              e.stopPropagation();
              if (openMenuEmail === account.email) {
                // 关闭菜单
                onSetOpenMenuEmail(null);
              } else {
                // 左键点击：以按钮 rect 计算视口坐标（后续用 portal + fixed 渲染）
                const rect = e.currentTarget.getBoundingClientRect();
                const position = getClampedMenuPosition(
                  rect.right - MENU_WIDTH,
                  rect.bottom + 4
                );
                setContextMenuPos({
                  x: position.x,
                  y: position.y,
                  isRightClick: false,
                });
                onSetOpenMenuEmail(account.email);
              }
            }}
            className="surface-secondary inline-flex items-center rounded-full border border-transparent px-4 py-2 text-xs font-medium text-slate-700 transition-colors hover:bg-slate-200/80 focus:outline-none focus:ring-2 focus:ring-gray-400 dark:text-slate-300 dark:hover:bg-slate-700/70"
          >
            更多
          </button>

          {/* 下拉菜单内容（portal 到 body，避免 transform/overflow 影响 fixed 定位） */}
          {openMenuEmail === account.email &&
            contextMenuPos &&
            createPortal(
              <div
                className="dropdown-menu panel-floating fixed z-[9999] w-48 rounded-md overflow-hidden"
                style={{
                  left: `${contextMenuPos.x}px`,
                  top: `${contextMenuPos.y}px`,
                }}
                onMouseDown={(e) => {
                  // 防止外层 document mousedown 监听把菜单当作“外部点击”而提前关闭
                  e.stopPropagation();
                }}
                onClick={(e) => {
                  e.stopPropagation();
                }}
              >
                <div className="py-1 text-slate-700 dark:text-slate-200">
                <button
                  type="button"
                  onClick={(e) => {
                    e.preventDefault();
                    e.stopPropagation();
                    onEditAccount(account);
                    onSetOpenMenuEmail(null);
                  }}
                  className="flex items-center w-full px-4 py-2 text-sm text-slate-700 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-800"
                >
                  ✏️ 编辑账户
                </button>
                <button
                  type="button"
                  onClick={(e) => {
                    e.preventDefault();
                    e.stopPropagation();
                    // 限制只保留第一个标签
                    const currentTags = account.custom_tags || [];
                    setEditingTags(currentTags.slice(0, 1));
                    setIsEditingTags(true);
                    onSetOpenMenuEmail(null);
                  }}
                  className="flex items-center w-full px-4 py-2 text-sm text-slate-700 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-800"
                >
                  🏷️ 自定义标签
                </button>
                {account.workos_cursor_session_token && (
                  <button
                    type="button"
                    onClick={(e) => {
                      e.preventDefault();
                      e.stopPropagation();
                      onViewUsage(account);
                      onSetOpenMenuEmail(null);
                    }}
                    className="flex items-center w-full px-4 py-2 text-sm text-slate-700 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-800"
                  >
                    📊 查看用量
                  </button>
                )}

                {/* 更新AccessToken按钮 */}
                {account.workos_cursor_session_token && (
                  <button
                    type="button"
                    onClick={(e) => {
                      e.preventDefault();
                      e.stopPropagation();
                      onUpdateAccessToken(account);
                      onSetOpenMenuEmail(null);
                    }}
                    disabled={updateAccessTokenLoading === account.email}
                    className="flex w-full items-center px-4 py-2 text-sm text-blue-700 hover:bg-blue-50 dark:text-blue-200 dark:hover:bg-blue-500/15 disabled:cursor-not-allowed disabled:opacity-50"
                  >
                    {updateAccessTokenLoading === account.email
                      ? "🔄 更新中..."
                      : "🔑 更新AccessToken"}
                  </button>
                )}

                {/* 重新登录按钮 - 仅对未授权账户显示 */}
                {account.auth_status === "unauthorized" && (
                  <button
                    type="button"
                    onClick={(e) => {
                      e.preventDefault();
                      e.stopPropagation();
                      onReLoginAccount(account);
                      onSetOpenMenuEmail(null);
                    }}
                    className="flex w-full items-center px-4 py-2 text-sm text-green-700 hover:bg-green-50 dark:text-green-200 dark:hover:bg-green-500/15"
                  >
                    🔄 重新登录
                  </button>
                )}

                {account.workos_cursor_session_token && (
                  <>
                    <button
                      type="button"
                      onClick={(e) => {
                        e.preventDefault();
                        e.stopPropagation();
                        onViewDashboard(account);
                        onSetOpenMenuEmail(null);
                      }}
                      className="flex items-center w-full px-4 py-2 text-sm text-slate-700 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-800"
                    >
                      🏠 查看主页
                    </button>
                    <button
                      type="button"
                      onClick={(e) => {
                        e.preventDefault();
                        e.stopPropagation();
                        onManualBindCard(account);
                        onSetOpenMenuEmail(null);
                      }}
                      disabled={manualBindCardLoading === account.email}
                      className="flex items-center w-full px-4 py-2 text-sm text-slate-700 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-800 disabled:opacity-50 disabled:cursor-not-allowed"
                    >
                      {manualBindCardLoading === account.email
                        ? "🔄 处理中..."
                        : "💳 手动绑卡"}
                    </button>

                    <button
                      type="button"
                      onClick={(e) => {
                        e.preventDefault();
                        e.stopPropagation();
                        onCopyBindCardUrl(account);
                        onSetOpenMenuEmail(null);
                      }}
                      className="flex items-center w-full px-4 py-2 text-sm text-slate-700 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-800"
                    >
                      📋 复制绑卡链接
                    </button>

                    <hr className="my-1" />

                    <button
                      type="button"
                      onClick={(e) => {
                        e.preventDefault();
                        e.stopPropagation();
                        onCancelSubscription(account);
                        onSetOpenMenuEmail(null);
                      }}
                      disabled={cancelSubscriptionLoading === account.email}
                      className="flex w-full items-center px-4 py-2 text-sm text-orange-700 hover:bg-orange-50 dark:text-orange-200 dark:hover:bg-orange-500/15 disabled:cursor-not-allowed disabled:opacity-50"
                    >
                      {cancelSubscriptionLoading === account.email
                        ? "🔄 处理中..."
                        : "📋 取消订阅"}
                    </button>
                    <button
                      type="button"
                      onClick={(e) => {
                        e.preventDefault();
                        e.stopPropagation();
                        onDeleteCursorAccount(account);
                        onSetOpenMenuEmail(null);
                      }}
                      className="flex w-full items-center px-4 py-2 text-sm text-red-700 hover:bg-red-50 dark:text-red-200 dark:hover:bg-red-500/15"
                    >
                      🚨 注销账户
                    </button>
                  </>
                )}
                </div>
              </div>,
              document.body
            )}
        </div>
      </div>

      {/* 使用进度条 */}
      {account.usage_progress && (
        <div className="mt-3 border-t border-slate-200 pt-3 dark:border-slate-700">
          <div className="flex items-center justify-between mb-2">
            <span className="text-xs font-medium text-slate-600 dark:text-slate-300">使用进度</span>
            <span
              className={`text-xs font-bold ${
                displayProgress >= 100
                  ? "text-red-600"
                  : displayProgress >= 80
                  ? "text-orange-600"
                  : "text-green-600"
              }`}
            >
              {Math.round(displayProgress)}%
            </span>
          </div>

          {/* 进度条 */}
          <div className="relative h-2 overflow-hidden rounded-full bg-slate-200 dark:bg-slate-800">
            <div
              className={`h-full ${
                shouldAnimate ? "transition-[width] duration-1000 ease-out" : ""
              } ${
                displayProgress >= 100
                  ? "bg-red-500"
                  : displayProgress >= 80
                  ? "bg-orange-500"
                  : displayProgress >= 50
                  ? "bg-yellow-500"
                  : "bg-green-500"
              }`}
              style={{
                width: `${displayProgress}%`,
                willChange: shouldAnimate ? "width" : "auto",
              }}
            />
          </div>

          {/* 额度信息 */}
          <div className="flex items-center justify-between mt-2">
            <div className="flex items-center space-x-3 text-xs">
              <span className="text-slate-600 dark:text-slate-300">
                已使用:{" "}
                <span className="font-semibold text-blue-600">
                  ${account.usage_progress.individualUsedDollars.toFixed(2)}
                </span>
              </span>
              <span className="text-slate-400 dark:text-slate-500">|</span>
              <span className="text-slate-600 dark:text-slate-300">
                剩余:{" "}
                <span className="font-semibold text-purple-600">
                  ${account.usage_progress.individualLimitDollars.toFixed(2)}
                </span>
              </span>
            </div>
            <span className="text-xs text-slate-500 dark:text-slate-400">
              总限额: $
              {account.usage_progress.percentage == 100
                ? account.usage_progress.individualLimitDollars.toFixed(2)
                : (
                    account.usage_progress.individualLimitDollars +
                    account.usage_progress.individualUsedDollars
                  ).toFixed(2)}
            </span>
          </div>
        </div>
      )}

      {/* 自定义标签编辑模态框 */}
      {isEditingTags && (
        createPortal(
          <div className="fixed inset-0 z-[10000] flex items-center justify-center bg-black/50 backdrop-blur-sm">
            <div className="panel-floating w-full max-w-md rounded-lg p-6">
              <h3 className="mb-4 text-lg font-semibold text-slate-900 dark:text-slate-100">
                自定义标签
              </h3>

              <div className="mb-4 max-h-64 space-y-3 overflow-y-auto">
                {editingTags.map((tag, index) => (
                  <div key={index} className="flex items-center gap-2">
                    <input
                      type="text"
                      value={tag.text}
                      onChange={(e) => {
                        const newTags = [...editingTags];
                        newTags[index].text = e.target.value.slice(0, 10);
                        setEditingTags(newTags);
                      }}
                      placeholder="标签文案（最多10字）"
                      maxLength={10}
                      className="field-input flex-1"
                    />
                    <input
                      type="color"
                      value={tag.color}
                      onChange={(e) => {
                        const newTags = [...editingTags];
                        newTags[index].color = e.target.value;
                        setEditingTags(newTags);
                      }}
                      aria-label="标签颜色"
                      className="h-10 w-12 cursor-pointer rounded-md border border-slate-300 bg-white dark:border-slate-600 dark:bg-slate-900"
                    />
                    <button
                      onClick={() => {
                        const newTags = editingTags.filter((_, i) => i !== index);
                        setEditingTags(newTags);
                      }}
                      className="rounded-md bg-red-100 px-2 py-2 text-sm text-red-700 hover:bg-red-200 dark:bg-red-500/15 dark:text-red-300 dark:hover:bg-red-500/25"
                    >
                      删除
                    </button>
                  </div>
                ))}
              </div>

              <button
                onClick={() => {
                  if (editingTags.length < 1) {
                    setEditingTags([
                      ...editingTags,
                      { text: "", color: "#3B82F6" },
                    ]);
                  }
                }}
                disabled={editingTags.length >= 1}
                className={`mb-4 w-full rounded-md px-4 py-2 text-sm ${
                  editingTags.length >= 1
                    ? "cursor-not-allowed bg-slate-100 text-slate-400 dark:bg-slate-800 dark:text-slate-500"
                    : "bg-blue-100 text-blue-700 hover:bg-blue-200 dark:bg-blue-500/15 dark:text-blue-200 dark:hover:bg-blue-500/25"
                }`}
              >
                + 添加标签 {editingTags.length >= 1 && "(最多1个)"}
              </button>

              <div className="flex justify-end gap-2">
                <button
                  onClick={() => {
                    setIsEditingTags(false);
                    // 恢复时也只保留第一个
                    setEditingTags((account.custom_tags || []).slice(0, 1));
                  }}
                  className="surface-secondary rounded-md px-4 py-2 text-sm text-slate-700 hover:bg-slate-200/80 dark:text-slate-300 dark:hover:bg-slate-700/70"
                >
                  取消
                </button>
                <button
                  onClick={async () => {
                    const { AccountService } = await import(
                      "../services/accountService"
                    );
                    // 只保留第一个非空标签（限制只能1个）
                    const filteredTags = editingTags
                      .filter((tag) => tag.text.trim() !== "")
                      .slice(0, 1);
                    const result = await AccountService.updateCustomTags(
                      account.email,
                      filteredTags
                    );

                    if (result.success) {
                      setIsEditingTags(false);
                      // 精准更新：直接更新前端数据
                      if (onUpdateAccountTags) {
                        onUpdateAccountTags(account.email, filteredTags);
                      }
                    } else {
                      alert("保存失败：" + result.message);
                    }
                  }}
                  className="rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700"
                >
                  保存
                </button>
              </div>
            </div>
          </div>,
          document.body
        )
      )}
    </div>
  );
};
