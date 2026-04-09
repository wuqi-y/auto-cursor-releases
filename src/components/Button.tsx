import React from "react";

interface ButtonProps {
  children: React.ReactNode;
  onClick?: (event?: React.MouseEvent<HTMLButtonElement>) => void;
  variant?: "primary" | "secondary" | "danger" | "success";
  size?: "sm" | "md" | "lg";
  disabled?: boolean;
  loading?: boolean;
  className?: string;
}

export const Button: React.FC<ButtonProps> = ({
  children,
  onClick,
  variant = "primary",
  size = "md",
  disabled = false,
  loading = false,
  className = "",
}) => {
  const baseClasses =
    "inline-flex items-center justify-center gap-2 rounded-xl border text-sm font-medium shadow-sm transition-all duration-200 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500/60 focus-visible:ring-offset-2 focus-visible:ring-offset-white disabled:pointer-events-none dark:focus-visible:ring-offset-slate-950";

  const variantClasses = {
    primary:
      "border-blue-500 bg-blue-600 text-white hover:-translate-y-0.5 hover:bg-blue-700 hover:shadow-lg hover:shadow-blue-500/20",
    secondary:
      "border-slate-200 bg-white text-slate-700 hover:-translate-y-0.5 hover:border-slate-300 hover:bg-slate-50 hover:shadow-md dark:border-slate-700 dark:bg-slate-900 dark:text-slate-200 dark:hover:border-slate-600 dark:hover:bg-slate-800",
    danger:
      "border-red-500 bg-red-600 text-white hover:-translate-y-0.5 hover:bg-red-700 hover:shadow-lg hover:shadow-red-500/20",
    success:
      "border-emerald-500 bg-emerald-600 text-white hover:-translate-y-0.5 hover:bg-emerald-700 hover:shadow-lg hover:shadow-emerald-500/20",
  };

  const sizeClasses = {
    sm: "px-3.5 py-2 text-xs",
    md: "px-4 py-2.5 text-sm",
    lg: "px-6 py-3 text-base",
  };

  const disabledClasses = "cursor-not-allowed opacity-55 shadow-none";

  const finalClasses = `
    ${baseClasses}
    ${variantClasses[variant]}
    ${sizeClasses[size]}
    ${disabled || loading ? disabledClasses : ""}
    ${className}
  `.trim();

  return (
    <button
      className={finalClasses}
      onClick={onClick}
      disabled={disabled || loading}
    >
      {loading && (
        <div className="w-4 h-4 mr-2 animate-spin rounded-full border-b-2 border-current"></div>
      )}
      {children}
    </button>
  );
};
