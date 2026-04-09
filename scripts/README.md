# 上传脚本使用说明

## 🚀 快速开始

### 1. 登录 GitHub CLI

首次使用需要登录：

```bash
gh auth login
```

按照提示选择：
1. GitHub.com
2. HTTPS
3. Login with a web browser（推荐）

### 2. 构建 macOS 包

```bash
# 同时构建 Intel 和 M1 两个版本
pnpm tauri:build:macos
```

### 3. 上传到 GitHub Release

```bash
# 指定版本号（必须）
pnpm upload:release 0.4.0

# 或者直接调用脚本
bash scripts/upload-to-release.sh 0.4.0
```

## 📋 完整工作流

### 方式一：自动化（推荐）

```bash
# 1. 构建所有 macOS 版本
pnpm tauri:build:macos

# 2. 上传到 Release（指定版本号）
pnpm upload:release 0.4.0
```

### 方式二：分步执行

```bash
# 1. 构建 Intel 版本
pnpm tauri:build:intel

# 2. 构建 M1 版本
pnpm tauri:build:m1

# 3. 上传（指定版本号）
pnpm upload:release 0.4.0
```

## 🔧 脚本功能

### 构建脚本 (`build-macos.sh`)
1. 💾 临时备份 Windows 和 Linux 的 pyBuild 文件到 `/tmp`
2. 🔨 构建 Intel 版本 (x86_64) - 只包含 macOS pyBuild
3. 🔨 构建 M1 版本 (aarch64) - 只包含 macOS pyBuild
4. 🔄 自动恢复备份的文件（即使构建失败也会恢复）

### 上传脚本 (`upload-to-release.sh`)
1. ✅ 检查 GitHub CLI 是否安装并登录
2. ✅ 检查构建文件是否存在
3. ✅ 验证目标 Release 是否存在
4. ✅ 上传 Intel 版本（auto-cursor_0.4.0_x64.dmg）
5. ✅ 上传 M1 版本（auto-cursor_0.4.0_aarch64.dmg）
6. ✅ 自动覆盖已存在的文件（--clobber）

## 📦 输出文件命名

脚本会上传以下文件到 GitHub Release：

- `auto-cursor_0.4.0_x64.dmg` - Intel 芯片版本
- `auto-cursor_0.4.0_aarch64.dmg` - Apple Silicon (M1/M2) 版本

## ⚠️ 注意事项

1. **安全的临时备份**：构建时会临时移动 Windows/Linux pyBuild 到 `/tmp`，构建完成后自动恢复
2. **源文件不会丢失**：使用 `trap` 机制确保即使构建失败也会恢复文件
3. **只包含 macOS pyBuild**：最终打包的 DMG 只包含 macOS 平台文件，体积更小
4. **Release 必须先存在**：上传前需要在 GitHub 手动创建 Release
5. **文件会被覆盖**：脚本使用 `--clobber` 参数，会覆盖已存在的同名文件
6. **需要写权限**：确保你的 GitHub token 有 repo 权限

## 🔗 相关链接

- Release 页面: https://github.com/wuqi-y/auto-cursor-releases/releases
- GitHub CLI 文档: https://cli.github.com/manual/

## 🐛 故障排除

### 问题：gh: command not found
```bash
# 安装 GitHub CLI
brew install gh
```

### 问题：You are not logged into any GitHub hosts
```bash
# 登录 GitHub
gh auth login
```

### 问题：Release 不存在
在 GitHub 上手动创建 Release：
https://github.com/wuqi-y/auto-cursor-releases/releases/new

### 问题：构建文件不存在
```bash
# 先构建
pnpm tauri:build:macos
```

