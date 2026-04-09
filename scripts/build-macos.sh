#!/bin/bash

# macOS 构建脚本 - 只包含 macOS 的 pyBuild 文件
set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}📦 开始构建 macOS 版本${NC}"
echo ""

# 临时备份其他平台的 pyBuild 文件
BACKUP_DIR="/tmp/auto-cursor-pybuild-backup-$$"
echo -e "${YELLOW}💾 备份其他平台的 pyBuild 文件到: ${BACKUP_DIR}${NC}"
mkdir -p "$BACKUP_DIR"

if [ -d "src-tauri/pyBuild/windows" ]; then
    mv src-tauri/pyBuild/windows "$BACKUP_DIR/"
    echo -e "${GREEN}✅ 已备份 Windows pyBuild${NC}"
fi
if [ -d "src-tauri/pyBuild/linux" ]; then
    mv src-tauri/pyBuild/linux "$BACKUP_DIR/"
    echo -e "${GREEN}✅ 已备份 Linux pyBuild${NC}"
fi
echo ""

# 清理函数 - 无论成功失败都会执行
cleanup() {
    echo ""
    echo -e "${YELLOW}🔄 恢复备份的 pyBuild 文件...${NC}"
    if [ -d "$BACKUP_DIR/windows" ]; then
        mv "$BACKUP_DIR/windows" src-tauri/pyBuild/
        echo -e "${GREEN}✅ 已恢复 Windows pyBuild${NC}"
    fi
    if [ -d "$BACKUP_DIR/linux" ]; then
        mv "$BACKUP_DIR/linux" src-tauri/pyBuild/
        echo -e "${GREEN}✅ 已恢复 Linux pyBuild${NC}"
    fi
    rm -rf "$BACKUP_DIR"
    echo -e "${GREEN}✅ 清理完成${NC}"
}

# 设置 trap 确保无论如何都会恢复文件
trap cleanup EXIT

# 构建 Intel 版本
echo -e "${YELLOW}🔨 构建 Intel 版本...${NC}"
pnpm tauri build --target x86_64-apple-darwin
echo -e "${GREEN}✅ Intel 版本构建完成${NC}"
echo ""

# 构建 M1 版本
echo -e "${YELLOW}🔨 构建 M1 版本...${NC}"
pnpm tauri build --target aarch64-apple-darwin
echo -e "${GREEN}✅ M1 版本构建完成${NC}"
echo ""

echo -e "${GREEN}🎉 所有构建完成！${NC}"
# cleanup 会在 EXIT 时自动执行

