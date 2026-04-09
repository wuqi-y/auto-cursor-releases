import { invoke } from "@tauri-apps/api/core";

export interface BackupInfo {
  cursor_settings: {
    exists: boolean;
    path: string;
    size?: number;
    lastModified?: string;
  };
  workspace_storage: {
    exists: boolean;
    path: string;
    size?: number;
    lastModified?: string;
    itemCount?: number;
  };
}

export interface BackupProgress {
  total: number;
  current: number;
  status: string;
  percentage: number;
}

export interface BackupListItem {
  name: string;
  created_at: string;
  size: number;
  type: 'full' | 'settings' | 'workspace';
}

export type BackupType = 'full' | 'settings' | 'workspace';

// 工作区详情相关接口
export interface WorkspaceStorageItem {
  id: string;
  name: string;
  path: string;
  workspaceInfo?: {
    folder: string;
  };
  conversationCount: number;
  lastModified: string;
  createdAt: string;
  size: number;
}

export interface ConversationData {
  id: string;
  title: string;
  lastMessage: string;
  createdAt: string;
  messageCount: number;
}

export interface ChatMessage {
  timestamp: string;
  sender: 'user' | 'assistant';
  content: string;
}

export interface ConversationDetail {
  id: string;
  title: string;
  createdAt: string;
  messageCount: number;
  messages: ChatMessage[];
}

export interface WorkspaceDetails {
  workspaceInfo: {
    folder: string;
  };
  conversations: ConversationData[];
  totalSize: number;
}

export class CursorBackupService {
  /**
   * 获取 Cursor 备份信息
   */
  static async getBackupInfo(): Promise<BackupInfo> {
    try {
      return await invoke<BackupInfo>("get_cursor_backup_info");
    } catch (error) {
      throw new Error(`获取 Cursor 备份信息失败: ${error}`);
    }
  }

  /**
   * 备份 Cursor 数据
   */
  static async backupData(type: BackupType): Promise<string> {
    try {
      return await invoke<string>("backup_cursor_data", { backupType: type });
    } catch (error) {
      throw new Error(`备份 Cursor 数据失败: ${error}`);
    }
  }

  /**
   * 恢复 Cursor 数据
   */
  static async restoreData(backupName: string): Promise<string> {
    try {
      return await invoke<string>("restore_cursor_data", { backupName });
    } catch (error) {
      throw new Error(`恢复 Cursor 数据失败: ${error}`);
    }
  }

  /**
   * 获取备份列表
   */
  static async getBackupList(): Promise<BackupListItem[]> {
    try {
      return await invoke<BackupListItem[]>("get_backup_list");
    } catch (error) {
      throw new Error(`获取备份列表失败: ${error}`);
    }
  }

  /**
   * 取消备份
   */
  static async cancelBackup(backupId: string): Promise<void> {
    try {
      return await invoke<void>("cancel_backup", { backupId });
    } catch (error) {
      throw new Error(`取消备份失败: ${error}`);
    }
  }

  /**
   * 删除备份
   */
  static async deleteBackup(backupName: string): Promise<string> {
    try {
      return await invoke<string>("delete_cursor_backup", { backupName });
    } catch (error) {
      throw new Error(`删除备份失败: ${error}`);
    }
  }

  /**
   * 格式化文件大小
   */
  static formatFileSize(bytes: number): string {
    const units = ['B', 'KB', 'MB', 'GB'];
    let size = bytes;
    let unitIndex = 0;

    while (size >= 1024 && unitIndex < units.length - 1) {
      size /= 1024;
      unitIndex++;
    }

    return `${size.toFixed(1)} ${units[unitIndex]}`;
  }

  /**
   * 格式化日期
   */
  static formatDate(dateString: string): string {
    if (!dateString || dateString === '未知时间') {
      return '未知时间';
    }

    try {
      // 处理后端返回的 "YYYY-MM-DD HH:MM:SS UTC" 格式
      let date: Date;
      if (dateString.includes(' UTC')) {
        // 替换 UTC 为标准格式
        const isoString = dateString.replace(' UTC', '+00:00').replace(' ', 'T');
        date = new Date(isoString);
      } else {
        date = new Date(dateString);
      }

      if (isNaN(date.getTime())) {
        return dateString; // 如果解析失败，返回原字符串
      }

      return date.toLocaleString('zh-CN');
    } catch (error) {
      console.warn('日期格式化失败:', dateString, error);
      return dateString;
    }
  }

  /**
   * 获取备份类型的显示名称
   */
  static getBackupTypeName(type: string): string {
    switch (type) {
      case 'full':
        return '完整备份';
      case 'settings':
        return '设置备份';
      case 'workspace':
        return '对话备份';
      default:
        return '未知类型';
    }
  }

  /**
   * 获取备份类型的颜色类
   */
  static getBackupTypeColor(type: string): string {
    switch (type) {
      case 'full':
        return 'bg-blue-100 text-blue-800';
      case 'settings':
        return 'bg-green-100 text-green-800';
      case 'workspace':
        return 'bg-purple-100 text-purple-800';
      default:
        return 'bg-gray-100 text-gray-800';
    }
  }

  /**
   * 打开 Cursor 设置目录
   */
  static async openSettingsDir(): Promise<string> {
    try {
      return await invoke<string>("open_cursor_settings_dir");
    } catch (error) {
      throw new Error(`打开设置目录失败: ${error}`);
    }
  }

  /**
   * 打开 Cursor 工作区目录
   */
  static async openWorkspaceDir(): Promise<string> {
    try {
      return await invoke<string>("open_cursor_workspace_dir");
    } catch (error) {
      throw new Error(`打开工作区目录失败: ${error}`);
    }
  }

  /**
   * 打开备份目录
   */
  static async openBackupDir(): Promise<string> {
    try {
      return await invoke<string>("open_backup_dir");
    } catch (error) {
      throw new Error(`打开备份目录失败: ${error}`);
    }
  }

  /**
   * 获取工作区存储项目列表
   */
  static async getWorkspaceStorageItems(): Promise<WorkspaceStorageItem[]> {
    try {
      return await invoke<WorkspaceStorageItem[]>("get_workspace_storage_items");
    } catch (error) {
      throw new Error(`获取工作区列表失败: ${error}`);
    }
  }

  /**
   * 获取工作区详情
   */
  static async getWorkspaceDetails(workspaceId: string): Promise<WorkspaceDetails> {
    try {
      return await invoke<WorkspaceDetails>("get_workspace_details", { workspaceId });
    } catch (error) {
      throw new Error(`获取工作区详情失败: ${error}`);
    }
  }

  /**
   * 调试工作区SQLite数据库（开发调试用）
   */
  static async debugWorkspaceSqlite(workspaceId: string): Promise<string> {
    try {
      return await invoke<string>("debug_workspace_sqlite", { workspaceId });
    } catch (error) {
      throw new Error(`调试SQLite失败: ${error}`);
    }
  }

  static async getConversationDetail(workspaceId: string, conversationId: string): Promise<ConversationDetail> {
    try {
      return await invoke<ConversationDetail>("get_conversation_detail", { workspaceId, conversationId });
    } catch (error) {
      throw new Error(`获取对话详情失败: ${error}`);
    }
  }
}
