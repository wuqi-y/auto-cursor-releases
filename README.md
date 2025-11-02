<h1 align="center">🚀 Auto Cursor - 专业的 Cursor IDE 管理工具</h1>

<br/>

<div align="center">


![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows-blue)
![License](https://img.shields.io/badge/license-MIT-green)
![Version](https://img.shields.io/badge/version-1.0.0-orange)

**一键管理您的 Cursor IDE 账户、订阅和使用量**

[📥 立即下载](https://github.com/wuqi-y/auto-cursor-releases/releases) · [📖 使用指南](https://github.com/wuqi-y/auto-cursor-releases/releases) · [🐛 问题反馈](https://github.com/wuqi-y/auto-cursor-releases/issues)

</div>

  <br/>
  <br/>

<img width="1546" height="1127" alt="image" src="https://github.com/user-attachments/assets/a5990d90-a8a3-4ffc-9edf-e3d2b2f413f8" />

### 支持备份cursor对话记录

### 最新版支持无感换号，无需重启cursor，以及支持自动轮换账户，账户到期自动轮换你当前账户可用的账户，实现真正的无感换号

### 其他功能下载查看

## 使用说明

### 1. 检查 Cursor 安装
应用启动时会自动检测系统中的 Cursor 编辑器安装。如果未检测到，会显示相应提示。

### 2. 选择备份文件
在主界面中，应用会列出所有可用的机器ID备份文件，包括：
- 文件名
- 创建日期
- 文件大小

### 3. 预览机器ID
选择备份文件后，可以预览其中包含的机器ID信息：
- `telemetry.devDeviceId`
- `telemetry.macMachineId` 
- `telemetry.machineId`
- `telemetry.sqmId`
- `storage.serviceMachineId`

### 4. 确认恢复
确认要恢复的机器ID后，应用会：
- 创建当前配置的备份
- 更新 storage.json 文件
- 更新 SQLite 数据库
- 更新 machineId 文件
- 更新系统级标识（如果有权限）

### 5. 完成恢复
恢复完成后，需要：
- 关闭 Cursor 编辑器
- 重新启动 Cursor 编辑器
- 检查编辑器是否正常工作

## 安全说明

- 应用只读取和修改 Cursor 相关的配置文件
- 系统级操作需要相应权限
- 所有操作前都会创建备份
- 不会收集或上传任何用户数据

## 常见问题

### Q: 为什么需要管理员权限？
A: 某些系统级ID更新（如Windows注册表、macOS系统配置）需要提升权限。

### Q: 恢复失败怎么办？
A: 应用会显示详细的错误信息，并且已创建的备份可以用于手动恢复。

### Q: 支持哪些备份文件格式？
A: 支持标准的 JSON 格式备份文件，文件名格式为 `storage.json.bak.YYYYMMDD_HHMMSS`。


<h1>📩 Disclaimer | 免责声明</h1>
本工具仅供学习和研究使用，使用本工具所产生的任何后果由使用者自行承担。

This tool is only for learning and research purposes, and any consequences arising from the use of this tool are borne by the user.


