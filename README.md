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

<img width="1000" height="700" alt="image" src="https://github.com/user-attachments/assets/7db1246e-ae69-4446-8fe9-35456f14c757" />

<div align="center">
  <br/>
  <h3>支持重置id以及备份恢复等</h3>
  <br/>
</div>

<img width="1000" height="700" alt="image" src="https://github.com/user-attachments/assets/5994578c-54a9-4a97-a154-071dde62d821" />

<div align="center">
  <br/>
  <h3>支持检查授权状态查看当前信息</h3>
  <br/>
</div>

<img width="871" height="1094" alt="image" src="https://github.com/user-attachments/assets/5549ae73-e015-41aa-b72e-06a70875327d" />



<div align="center">
  <br/>
  <h3>方便查看token使用量</h3>
  <br/>
</div>

<img width="1000" height="700" alt="image" src="https://github.com/user-attachments/assets/801a1c98-911c-4337-83e0-c2c465bff203" />
<img width="1000" height="700" alt="image" src="https://github.com/user-attachments/assets/36bf6098-9c10-4300-aff3-6dbf54215d50" />

<div align="center">
  <br/>
  <h3>快捷添加和切换账号方便管理</h3>
  <h3>注销账户：一键删除cursor账户；取消订阅：自动打开新窗口注入登录态跳转到取消订阅页面，需要手动点击取消；切换账号会自动重置机器id</h3>
  <br/>
</div>

<img width="933" height="539" alt="image" src="https://github.com/user-attachments/assets/822d1185-a77b-4b2c-98a2-9c773e6171c6" />

<img width="1026" height="902" alt="image" src="https://github.com/user-attachments/assets/5594980c-71e8-4af5-a4e1-2e8efbf6c85b" />

<div align="center">
  <br/>
  <h3>支持自动注册-自动获取验证码-自动绑卡-绑卡后自动添加账号并获取 accessToken 和 WorkosCursorSessionToken（自动注册部分代码参考 <a href="https://github.com/yeongpin/cursor-free-vip" target="_blank">cursor-free-vip</a>）</h3>
  <br/>
</div>

 -- 自动获取验证码暂时只支持 <a href="https://github.com/dreamhunter2333/cloudflare_temp_email" target="_blank">cloudflare_temp_email</a>

<img width="1026" height="902" alt="image" src="https://github.com/user-attachments/assets/4df4624f-f93c-4a35-8517-b4708571c240" />

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


