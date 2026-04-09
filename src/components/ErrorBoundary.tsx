import { Component, ErrorInfo, ReactNode } from "react";
import { WebLogService } from "../services/weblogService";

interface Props {
  children: ReactNode;
  fallback?: ReactNode;
}

interface State {
  hasError: boolean;
  error?: Error;
}

export class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false };
  }

  static getDerivedStateFromError(error: Error): State {
    // 更新 state 使下一次渲染能够显示降级后的 UI
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    // 记录错误到 weblog
    WebLogService.logError("REACT_ERROR_BOUNDARY", error.message, {
      stack: error.stack,
      componentStack: errorInfo.componentStack,
      url: window.location.href,
      timestamp: new Date().toISOString(),
    });

    console.error("ErrorBoundary caught an error:", error, errorInfo);
  }

  render() {
    if (this.state.hasError) {
      // 你可以自定义降级后的 UI 并渲染
      return (
        this.props.fallback || (
          <div
            className="error-boundary-fallback"
            style={{
              padding: "20px",
              margin: "20px",
              border: "1px solid #ff6b6b",
              borderRadius: "8px",
              backgroundColor: "#ffe0e0",
              color: "#d63031",
              textAlign: "center",
            }}
          >
            <h2>🚨 出现了一个错误</h2>
            <p>抱歉，应用程序遇到了一个意外错误。</p>
            <details style={{ marginTop: "10px", textAlign: "left" }}>
              <summary style={{ cursor: "pointer", fontWeight: "bold" }}>
                点击查看错误详情
              </summary>
              <pre
                style={{
                  marginTop: "10px",
                  padding: "10px",
                  backgroundColor: "#f8f9fa",
                  border: "1px solid #dee2e6",
                  borderRadius: "4px",
                  fontSize: "12px",
                  overflow: "auto",
                  maxHeight: "200px",
                }}
              >
                {this.state.error?.stack}
              </pre>
            </details>
            <button
              onClick={() => {
                this.setState({ hasError: false, error: undefined });
                window.location.reload();
              }}
              style={{
                marginTop: "15px",
                padding: "8px 16px",
                backgroundColor: "#0984e3",
                color: "white",
                border: "none",
                borderRadius: "4px",
                cursor: "pointer",
              }}
            >
              重新加载页面
            </button>
          </div>
        )
      );
    }

    return this.props.children;
  }
}

export default ErrorBoundary;
