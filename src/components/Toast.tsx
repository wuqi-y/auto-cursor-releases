import React, { useState, useEffect } from "react";

export interface ToastProps {
  message: string;
  type?: "success" | "error" | "warning" | "info";
  duration?: number;
  onClose?: () => void;
}

export const Toast: React.FC<ToastProps> = ({
  message,
  type = "info",
  duration = 3000,
  onClose
}) => {
  const [isVisible, setIsVisible] = useState(true);

  useEffect(() => {
    const timer = setTimeout(() => {
      setIsVisible(false);
      setTimeout(() => onClose?.(), 300); // 等待动画完成
    }, duration);

    return () => clearTimeout(timer);
  }, [duration, onClose]);

  const typeStyles = {
    success:
      "border-emerald-200 bg-white/95 text-emerald-700 shadow-emerald-500/10 dark:border-emerald-500/30 dark:bg-slate-900/95 dark:text-emerald-300",
    error:
      "border-red-200 bg-white/95 text-red-700 shadow-red-500/10 dark:border-red-500/30 dark:bg-slate-900/95 dark:text-red-300",
    warning:
      "border-amber-200 bg-white/95 text-amber-700 shadow-amber-500/10 dark:border-amber-500/30 dark:bg-slate-900/95 dark:text-amber-300",
    info:
      "border-blue-200 bg-white/95 text-blue-700 shadow-blue-500/10 dark:border-blue-500/30 dark:bg-slate-900/95 dark:text-blue-300"
  };

  const icons = {
    success: "✅",
    error: "❌",
    warning: "⚠️",
    info: "ℹ️"
  };

  return (
    <div
      className={`fixed top-4 right-4 z-50 min-w-[280px] max-w-sm rounded-2xl border px-4 py-3 shadow-2xl backdrop-blur-xl transition-all duration-300 ${
        isVisible ? "translate-y-0 opacity-100" : "-translate-y-2 opacity-0"
      } ${typeStyles[type]}`}
    >
      <div className="flex items-start gap-3">
        <span className="mt-0.5 text-lg">{icons[type]}</span>
        <div className="flex-1">
          <p className="text-sm font-semibold">{message}</p>
        </div>
        <button
          onClick={() => {
            setIsVisible(false);
            setTimeout(() => onClose?.(), 300);
          }}
          className="text-base leading-none opacity-70 transition-opacity hover:opacity-100"
        >
          ×
        </button>
      </div>
    </div>
  );
};

// Toast 管理器
export interface ToastItem extends ToastProps {
  id: string;
}

interface ToastManagerProps {
  toasts: ToastItem[];
  removeToast: (id: string) => void;
}

export const ToastManager: React.FC<ToastManagerProps> = ({ toasts, removeToast }) => {
  return (
    <div className="fixed top-4 right-4 z-50 space-y-2">
      {toasts.map((toast) => (
        <Toast
          key={toast.id}
          {...toast}
          onClose={() => removeToast(toast.id)}
        />
      ))}
    </div>
  );
};

// Toast Hook
export const useToast = () => {
  const [toasts, setToasts] = useState<ToastItem[]>([]);

  const addToast = (toast: Omit<ToastItem, "id">) => {
    const id = Math.random().toString(36).substr(2, 9);
    setToasts(prev => [...prev, { ...toast, id }]);
  };

  const removeToast = (id: string) => {
    setToasts(prev => prev.filter(toast => toast.id !== id));
  };

  const showSuccess = (message: string, duration?: number) => {
    addToast({ message, type: "success", duration });
  };

  const showError = (message: string, duration?: number) => {
    addToast({ message, type: "error", duration });
  };

  const showWarning = (message: string, duration?: number) => {
    addToast({ message, type: "warning", duration });
  };

  const showInfo = (message: string, duration?: number) => {
    addToast({ message, type: "info", duration });
  };

  return {
    toasts,
    removeToast,
    showSuccess,
    showError,
    showWarning,
    showInfo
  };
};
