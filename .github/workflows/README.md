# GitHub Actions 构建工作流

本项目包含两个GitHub Actions工作流，用于在多个平台上构建Tauri应用程序。

## 主要构建工作流 (`build.yml`)

### 触发条件
- 推送标签 (格式: `v*`)
- 手动触发

### 支持的平台
1. **macOS Intel** (`x86_64-apple-darwin`)
   - 生成: `.dmg` 和 `.app.tar.gz` 文件
   
2. **macOS Apple Silicon** (`aarch64-apple-darwin`)
   - 生成: `.dmg` 和 `.app.tar.gz` 文件
   
3. **Windows x64** (`x86_64-pc-windows-msvc`)
   - 生成: `.msi` 和 `.exe` 文件
   
4. **Linux x64** (`x86_64-unknown-linux-gnu`)
   - 生成: `.deb`, `.rpm` 和 `.AppImage` 文件

### 构建产物
- 构建产物会上传为 GitHub Artifacts
- 文件命名格式: `auto-cursor-{os}-{arch}`
- 如果推送的是标签，会自动创建 GitHub Release

## 快速构建工作流 (`quick-build.yml`)

### 触发条件
- 仅手动触发
- 可以选择特定平台进行构建

### 用途
- 快速测试单个平台的构建
- 调试构建问题
- 节省CI资源

## 使用方法

### 1. 发布版本
创建格式为 `v*` 的标签会触发构建并创建 GitHub Release：

```bash
git tag v1.0.0
git push origin v1.0.0
```

### 2. 手动构建
在 GitHub 项目页面进入 Actions 标签页，选择对应的工作流并手动触发。

## 构建要求

### 依赖项
- Node.js 20
- pnpm 8
- Rust (stable)
- Python 3 (用于处理 pyBuild 目录)

### 平台特定依赖
- **Linux**: 需要安装 GTK 和 WebKit 相关库
- **macOS**: 无额外要求
- **Windows**: 无额外要求

## 构建缓存

工作流包含以下缓存优化：
- pnpm 依赖缓存
- Rust 编译缓存
- 跨构建复用以提高速度

## 故障排除

### 常见问题
1. **构建失败**: 检查依赖项是否正确安装
2. **文件未找到**: 确保 `src-tauri/pyBuild` 目录存在且包含必要文件
3. **权限错误**: 确保 `GITHUB_TOKEN` 权限正确

### 调试建议
1. 使用快速构建工作流测试单个平台
2. 检查构建日志中的错误信息
3. 验证本地构建是否成功

## 自定义配置

如需修改构建配置，可以编辑：
- `build.yml`: 主要构建流程
- `quick-build.yml`: 快速构建选项
- `src-tauri/tauri.conf.json`: Tauri 应用配置
