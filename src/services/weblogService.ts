import { invoke } from '@tauri-apps/api/core';

export interface WebLogEntry {
  level: string;
  message: string;
  url?: string;
  user_agent?: string;
  stack?: string;
}

export class WebLogService {
  private static isInitialized = false;

  /**
   * 初始化全局错误监听器
   */
  static init() {
    if (this.isInitialized) {
      return;
    }

    // 监听未捕获的 JavaScript 错误
    window.addEventListener('error', (event) => {
      this.logError('JAVASCRIPT_ERROR', event.message, {
        url: window.location.href,
        filename: event.filename,
        lineno: event.lineno,
        colno: event.colno,
        stack: event.error?.stack,
      });
    });

    // 监听未捕获的 Promise 拒绝
    window.addEventListener('unhandledrejection', (event) => {
      this.logError('PROMISE_REJECTION', event.reason?.message || String(event.reason), {
        url: window.location.href,
        stack: event.reason?.stack,
      });
    });

    // 监听 React 错误边界（如果使用了错误边界）
    const originalConsoleError = console.error;
    console.error = (...args) => {
      // 检查是否是 React 错误
      const message = args.join(' ');
      if (message.includes('React') || message.includes('component')) {
        this.logError('REACT_ERROR', message, {
          url: window.location.href,
          stack: new Error().stack,
        });
      }
      originalConsoleError.apply(console, args);
    };

    // 监听网络请求错误
    const originalFetch = window.fetch;
    window.fetch = async (...args) => {
      try {
        const response = await originalFetch(...args);
        if (!response.ok) {
          this.logError('NETWORK_ERROR', `HTTP ${response.status}: ${response.statusText}`, {
            url: window.location.href,
            requestUrl: typeof args[0] === 'string' ? args[0] : (args[0] as Request)?.url || 'unknown',
          });
        }
        return response;
      } catch (error) {
        this.logError('NETWORK_ERROR', `Fetch failed: ${error}`, {
          url: window.location.href,
          requestUrl: typeof args[0] === 'string' ? args[0] : (args[0] as Request)?.url || 'unknown',
          stack: error instanceof Error ? error.stack : undefined,
        });
        throw error;
      }
    };

    this.isInitialized = true;
    this.logInfo('WebLog service initialized successfully');
  }

  /**
   * 记录错误日志
   */
  static async logError(type: string, message: string, details?: any) {
    const fullMessage = details ? `${type}: ${message} | Details: ${JSON.stringify(details)}` : `${type}: ${message}`;

    await this.writeLog('ERROR', fullMessage, {
      url: details?.url || window.location.href,
      user_agent: navigator.userAgent,
      stack: details?.stack,
    });
  }

  /**
   * 记录警告日志
   */
  static async logWarn(message: string, details?: any) {
    const fullMessage = details ? `${message} | Details: ${JSON.stringify(details)}` : message;

    await this.writeLog('WARN', fullMessage, {
      url: window.location.href,
      user_agent: navigator.userAgent,
    });
  }

  /**
   * 记录信息日志
   */
  static async logInfo(message: string, details?: any) {
    const fullMessage = details ? `${message} | Details: ${JSON.stringify(details)}` : message;

    await this.writeLog('INFO', fullMessage, {
      url: window.location.href,
      user_agent: navigator.userAgent,
    });
  }

  /**
   * 记录调试日志
   */
  static async logDebug(message: string, details?: any) {
    const fullMessage = details ? `${message} | Details: ${JSON.stringify(details)}` : message;

    await this.writeLog('DEBUG', fullMessage, {
      url: window.location.href,
      user_agent: navigator.userAgent,
    });
  }

  /**
   * 写入日志到后端
   */
  private static async writeLog(level: string, message: string, options: {
    url?: string;
    user_agent?: string;
    stack?: string;
  } = {}) {
    try {
      await invoke('write_weblog', {
        level,
        message,
        url: options.url,
        userAgent: options.user_agent,
        stack: options.stack,
      });
    } catch (error) {
      // 如果写入日志失败，至少输出到控制台
      console.error('Failed to write web log:', error);
      console.log(`[WEBLOG] [${level}] ${message}`);
    }
  }

  /**
   * 获取最近的日志
   */
  static async getRecentLogs(limit: number = 100): Promise<string[]> {
    try {
      return await invoke('get_recent_weblogs', { limit });
    } catch (error) {
      console.error('Failed to get recent web logs:', error);
      return [];
    }
  }

  /**
   * 获取日志文件路径
   */
  static async getLogFilePath(): Promise<string> {
    try {
      return await invoke('get_weblog_file_path');
    } catch (error) {
      console.error('Failed to get web log file path:', error);
      return '';
    }
  }

  /**
   * 获取日志配置
   */
  static async getLogConfig(): Promise<any> {
    try {
      return await invoke('get_weblog_config');
    } catch (error) {
      console.error('Failed to get web log config:', error);
      return null;
    }
  }

  /**
   * 手动记录用户操作日志
   */
  static async logUserAction(action: string, details?: any) {
    await this.logInfo(`USER_ACTION: ${action}`, details);
  }

  /**
   * 记录页面访问日志
   */
  static async logPageVisit(page: string) {
    await this.logInfo(`PAGE_VISIT: ${page}`, {
      timestamp: new Date().toISOString(),
      referrer: document.referrer,
    });
  }

  /**
   * 记录性能相关日志
   */
  static async logPerformance(metric: string, value: number, unit: string = 'ms') {
    await this.logInfo(`PERFORMANCE: ${metric} = ${value}${unit}`);
  }
}

// 导出便捷的全局函数
export const weblog = {
  error: WebLogService.logError.bind(WebLogService),
  warn: WebLogService.logWarn.bind(WebLogService),
  info: WebLogService.logInfo.bind(WebLogService),
  debug: WebLogService.logDebug.bind(WebLogService),
  userAction: WebLogService.logUserAction.bind(WebLogService),
  pageVisit: WebLogService.logPageVisit.bind(WebLogService),
  performance: WebLogService.logPerformance.bind(WebLogService),
};
