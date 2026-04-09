import React from "react";
import { Button } from "./Button";

export interface ConfirmDialogProps {
  isOpen: boolean;
  title: string;
  message: string;
  confirmText?: string;
  cancelText?: string;
  type?: "danger" | "warning" | "info";
  onConfirm: (checkboxValue?: boolean, autoCloseValue?: boolean) => void;
  onCancel: () => void;
  loading?: boolean;
  checkboxLabel?: string;
  checkboxDefaultChecked?: boolean;
  checkboxDisabled?: boolean;
  onCheckboxChange?: (checked: boolean) => void;
  autoCloseCheckboxLabel?: string;
  autoCloseCheckboxDefaultChecked?: boolean;
  autoCloseCheckboxDisabled?: boolean;
  onAutoCloseCheckboxChange?: (checked: boolean) => void;
}

export const ConfirmDialog: React.FC<ConfirmDialogProps> = ({
  isOpen,
  title,
  message,
  confirmText = "确认",
  cancelText = "取消",
  type = "warning",
  onConfirm,
  onCancel,
  loading = false,
  checkboxLabel,
  checkboxDefaultChecked = true,
  checkboxDisabled = false,
  onCheckboxChange,
  autoCloseCheckboxLabel,
  autoCloseCheckboxDefaultChecked = true,
  autoCloseCheckboxDisabled = false,
  onAutoCloseCheckboxChange,
}) => {
  const [checked, setChecked] = React.useState(checkboxDefaultChecked);
  const [autoCloseChecked, setAutoCloseChecked] = React.useState(
    autoCloseCheckboxDefaultChecked
  );
  const [isConfirmDisabled, setIsConfirmDisabled] = React.useState(false);

  React.useEffect(() => {
    setChecked(checkboxDefaultChecked);
    setAutoCloseChecked(autoCloseCheckboxDefaultChecked);
    setIsConfirmDisabled(false);
  }, [checkboxDefaultChecked, autoCloseCheckboxDefaultChecked, isOpen]);

  const handleCheckboxChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const newChecked = e.target.checked;
    setChecked(newChecked);
    onCheckboxChange?.(newChecked);
  };

  const handleAutoCloseCheckboxChange = (
    e: React.ChangeEvent<HTMLInputElement>
  ) => {
    const newChecked = e.target.checked;
    setAutoCloseChecked(newChecked);
    onAutoCloseCheckboxChange?.(newChecked);
  };

  const handleConfirmClick = () => {
    setIsConfirmDisabled(true);
    onConfirm(
      checkboxLabel ? checked : undefined,
      autoCloseCheckboxLabel ? autoCloseChecked : undefined
    );
  };

  if (!isOpen) return null;

  const typeStyles = {
    danger: {
      icon: "🚨",
      iconBg: "bg-red-100 text-red-600 dark:bg-red-500/10 dark:text-red-300",
      confirmVariant: "danger" as const,
    },
    warning: {
      icon: "⚠️",
      iconBg: "bg-amber-100 text-amber-600 dark:bg-amber-500/10 dark:text-amber-300",
      confirmVariant: "secondary" as const,
    },
    info: {
      icon: "ℹ️",
      iconBg: "bg-blue-100 text-blue-600 dark:bg-blue-500/10 dark:text-blue-300",
      confirmVariant: "primary" as const,
    },
  };

  const style = typeStyles[type];

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center px-4">
      <div
        className="absolute inset-0 bg-slate-950/55 backdrop-blur-sm"
        onClick={onCancel}
      />

      <div className="relative w-full max-w-md rounded-[28px] border border-white/70 bg-white/92 shadow-2xl backdrop-blur-xl dark:border-slate-800/80 dark:bg-slate-900/92">
        <div className="p-6">
          <div className="mb-5 flex items-center gap-3">
            <div
              className={`flex h-11 w-11 flex-shrink-0 items-center justify-center rounded-2xl ${style.iconBg}`}
            >
              <span className="text-lg">{style.icon}</span>
            </div>
            <h3 className="text-lg font-semibold text-slate-900 dark:text-slate-100">
              {title}
            </h3>
          </div>

          <div className="mb-6">
            <p className="text-sm leading-6 text-slate-600 dark:text-slate-300">
              {message}
            </p>
          </div>

          {(checkboxLabel || autoCloseCheckboxLabel) && (
            <div className="mb-6 space-y-3 rounded-2xl border border-slate-200/80 bg-slate-50/80 p-4 dark:border-slate-800 dark:bg-slate-800/60">
              {checkboxLabel && (
                <label
                  className={`flex items-center ${
                    checkboxDisabled ? "cursor-not-allowed" : "cursor-pointer"
                  }`}
                >
                  <input
                    type="checkbox"
                    checked={checked}
                    onChange={handleCheckboxChange}
                    disabled={checkboxDisabled}
                    className={`h-4 w-4 rounded border-slate-300 text-blue-600 focus:ring-blue-500 ${
                      checkboxDisabled ? "cursor-not-allowed opacity-50" : ""
                    }`}
                  />
                  <span
                    className={`ml-2 text-sm ${
                      checkboxDisabled
                        ? "text-slate-400"
                        : "text-slate-700 dark:text-slate-300"
                    }`}
                  >
                    {checkboxLabel}
                  </span>
                </label>
              )}
              {autoCloseCheckboxLabel && (
                <label
                  className={`flex items-center ${
                    autoCloseCheckboxDisabled
                      ? "cursor-not-allowed"
                      : "cursor-pointer"
                  }`}
                >
                  <input
                    type="checkbox"
                    checked={autoCloseChecked}
                    onChange={handleAutoCloseCheckboxChange}
                    disabled={autoCloseCheckboxDisabled}
                    className={`h-4 w-4 rounded border-slate-300 text-blue-600 focus:ring-blue-500 ${
                      autoCloseCheckboxDisabled
                        ? "cursor-not-allowed opacity-50"
                        : ""
                    }`}
                  />
                  <span
                    className={`ml-2 text-sm ${
                      autoCloseCheckboxDisabled
                        ? "text-slate-400"
                        : "text-slate-700 dark:text-slate-300"
                    }`}
                  >
                    {autoCloseCheckboxLabel}
                  </span>
                </label>
              )}
            </div>
          )}

          <div className="flex justify-end gap-3">
            <Button
              variant="secondary"
              onClick={onCancel}
              disabled={loading || isConfirmDisabled}
            >
              {cancelText}
            </Button>
            <Button
              variant={style.confirmVariant}
              onClick={handleConfirmClick}
              loading={loading}
              disabled={isConfirmDisabled}
            >
              {confirmText}
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
};

export const useConfirmDialog = () => {
  const [dialog, setDialog] = React.useState<{
    isOpen: boolean;
    title: string;
    message: string;
    confirmText?: string;
    cancelText?: string;
    type?: "danger" | "warning" | "info";
    onConfirm?: () => void;
    loading?: boolean;
  }>({
    isOpen: false,
    title: "",
    message: "",
  });

  const showConfirm = (options: {
    title: string;
    message: string;
    confirmText?: string;
    cancelText?: string;
    type?: "danger" | "warning" | "info";
    onConfirm: () => void;
  }) => {
    setDialog({
      isOpen: true,
      ...options,
      loading: false,
    });
  };

  const hideConfirm = () => {
    setDialog((prev) => ({ ...prev, isOpen: false }));
  };

  const setLoading = (loading: boolean) => {
    setDialog((prev) => ({ ...prev, loading }));
  };

  const handleConfirm = async () => {
    if (dialog.onConfirm) {
      setLoading(true);
      try {
        await dialog.onConfirm();
        hideConfirm();
      } catch (error) {
        console.error("Confirm dialog error:", error);
      } finally {
        setLoading(false);
      }
    }
  };

  const ConfirmDialogComponent = () => (
    <ConfirmDialog
      isOpen={dialog.isOpen}
      title={dialog.title}
      message={dialog.message}
      confirmText={dialog.confirmText}
      cancelText={dialog.cancelText}
      type={dialog.type}
      onConfirm={handleConfirm}
      onCancel={hideConfirm}
      loading={dialog.loading}
    />
  );

  return {
    showConfirm,
    hideConfirm,
    setLoading,
    ConfirmDialog: ConfirmDialogComponent,
  };
};
