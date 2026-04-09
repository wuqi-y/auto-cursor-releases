import React, { useEffect } from "react";
import ReactMarkdown from "react-markdown";
import { UpdateInfo } from "../types/update";
import { openUpdateUrl } from "../services/updateService";
import { Button } from "./Button";

interface UpdateModalProps {
  updateInfo: UpdateInfo;
  onClose: () => void;
}

export const UpdateModal: React.FC<UpdateModalProps> = ({
  updateInfo,
  onClose,
}) => {
  useEffect(() => {
    const originalStyle = {
      overflow: document.body.style.overflow,
      paddingRight: document.body.style.paddingRight,
    };

    const scrollBarWidth =
      window.innerWidth - document.documentElement.clientWidth;

    document.body.style.overflow = "hidden";
    document.body.style.paddingRight = `${scrollBarWidth}px`;

    return () => {
      document.body.style.overflow = originalStyle.overflow;
      document.body.style.paddingRight = originalStyle.paddingRight;
    };
  }, []);

  const handleUpdate = async () => {
    try {
      await openUpdateUrl(updateInfo.updateUrl);
    } catch (error) {
      console.error("Failed to open update URL:", error);
    }
  };

  const handleClose = () => {
    if (!updateInfo.isForceUpdate) {
      onClose();
    }
  };

  const formatDate = (dateString: string) => {
    const date = new Date(dateString);
    return date.toLocaleString("zh-CN", {
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
    });
  };

  return (
    <div className="update-modal-backdrop fixed inset-0 z-50 flex items-center justify-center">
      <div className="update-modal-content mx-4 flex max-h-[82vh] w-full max-w-2xl flex-col overflow-hidden rounded-[28px] border border-white/70 bg-white/92 shadow-2xl backdrop-blur-xl dark:border-slate-800/80 dark:bg-slate-900/92">
        <div className="flex items-center justify-between border-b border-slate-200/80 p-6 dark:border-slate-800/80">
          <div className="flex items-center gap-3">
            <div className="flex h-11 w-11 items-center justify-center rounded-2xl bg-blue-600/10 text-blue-600 dark:bg-blue-500/10 dark:text-blue-300">
              <svg className="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4"
                />
              </svg>
            </div>
            <div>
              <h3 className="text-lg font-semibold text-slate-900 dark:text-slate-100">
                {updateInfo.isForceUpdate ? "强制更新" : "发现新版本"}
              </h3>
              <p className="text-sm text-slate-500 dark:text-slate-400">
                版本 {updateInfo.version}
              </p>
            </div>
          </div>
          {!updateInfo.isForceUpdate && (
            <button
              onClick={handleClose}
              className="rounded-xl p-2 text-slate-400 transition-colors hover:bg-slate-100 hover:text-slate-700 dark:hover:bg-slate-800 dark:hover:text-slate-200"
              title="关闭"
              aria-label="关闭更新弹窗"
            >
              <svg className="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M6 18L18 6M6 6l12 12"
                />
              </svg>
            </button>
          )}
        </div>

        <div className="flex-1 space-y-5 overflow-y-auto p-6">
          <div className="rounded-2xl border border-slate-200/80 bg-slate-50/90 px-4 py-3 text-sm text-slate-600 dark:border-slate-800 dark:bg-slate-800/70 dark:text-slate-300">
            更新时间：{formatDate(updateInfo.updateDate)}
          </div>

          {updateInfo.isForceUpdate && (
            <div className="force-update-warning rounded-2xl p-4">
              <div className="flex items-center gap-3">
                <svg className="h-5 w-5 flex-shrink-0 text-red-600 dark:text-red-300" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4c-.77-.833-1.964-.833-2.732 0L3.732 16.5c-.77.833.192 2.5 1.732 2.5z"
                  />
                </svg>
                <span className="text-sm font-semibold text-red-800 dark:text-red-200">
                  这是一个强制更新，必须更新后才能继续使用。
                </span>
              </div>
            </div>
          )}

          <div>
            <h4 className="mb-3 text-sm font-semibold text-slate-900 dark:text-slate-100">
              更新内容
            </h4>
            <div className="rounded-2xl border border-slate-200/80 bg-white/70 p-4 dark:border-slate-800 dark:bg-slate-950/40">
              <div className="prose prose-sm max-w-none text-slate-700 dark:prose-invert dark:text-slate-300">
                <ReactMarkdown
                  components={{
                    p: ({ children }) => <p className="mb-2 last:mb-0">{children}</p>,
                    ul: ({ children }) => <ul className="mb-2 list-disc list-inside">{children}</ul>,
                    ol: ({ children }) => <ol className="mb-2 list-decimal list-inside">{children}</ol>,
                    li: ({ children }) => <li className="mb-1">{children}</li>,
                    strong: ({ children }) => <strong className="font-semibold">{children}</strong>,
                    em: ({ children }) => <em className="italic">{children}</em>,
                    code: ({ children }) => (
                      <code className="rounded bg-slate-100 px-1.5 py-0.5 text-sm font-mono text-slate-800 dark:bg-slate-800 dark:text-slate-200">
                        {children}
                      </code>
                    ),
                  }}
                >
                  {updateInfo.description}
                </ReactMarkdown>
              </div>
            </div>
          </div>
        </div>

        <div className="flex justify-end gap-3 border-t border-slate-200/80 p-6 dark:border-slate-800/80">
          {!updateInfo.isForceUpdate && (
            <Button variant="secondary" onClick={handleClose}>
              稍后更新
            </Button>
          )}
          <Button variant="primary" onClick={handleUpdate}>
            立即更新
          </Button>
        </div>
      </div>
    </div>
  );
};
