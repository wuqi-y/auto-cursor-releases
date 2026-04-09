#!/bin/bash

# 上传 macOS 构建产物到 GitHub Release
# 使用方法: ./scripts/upload-to-release.sh <version>
# 示例: ./scripts/upload-to-release.sh 0.4.0

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# 参数
VERSION=${1}
REPO="wuqi-y/auto-cursor-releases"

# 检查版本号参数
if [ -z "$VERSION" ]; then
    echo -e "${RED}❌ 错误: 缺少版本号参数${NC}"
    echo "使用方法: $0 <version>"
    echo "示例: $0 0.4.0"
    exit 1
fi

# 添加 v 前缀
TAG="v${VERSION}"

echo -e "${GREEN}📦 上传 macOS 构建产物到 GitHub Release${NC}"
echo -e "${YELLOW}标签: ${TAG}${NC}"
echo -e "${YELLOW}仓库: ${REPO}${NC}"
echo ""

# 检查 gh 是否安装
if ! command -v gh &> /dev/null; then
    echo -e "${RED}❌ 错误: GitHub CLI (gh) 未安装${NC}"
    echo "请运行: brew install gh"
    exit 1
fi

# 检查是否登录
if ! gh auth status &> /dev/null; then
    echo -e "${RED}❌ 错误: 未登录 GitHub CLI${NC}"
    echo "请运行: gh auth login"
    exit 1
fi

# 定义文件路径（使用版本号动态构建）
INTEL_DMG="src-tauri/target/x86_64-apple-darwin/release/bundle/dmg/auto-cursor_${VERSION}_x64.dmg"
M1_DMG="src-tauri/target/aarch64-apple-darwin/release/bundle/dmg/auto-cursor_${VERSION}_aarch64.dmg"

# 检查 DMG 文件是否存在
if [ ! -f "$INTEL_DMG" ]; then
    echo -e "${RED}❌ 错误: Intel 版本不存在: $INTEL_DMG${NC}"
    echo "请先运行: pnpm tauri:build:intel"
    exit 1
fi

if [ ! -f "$M1_DMG" ]; then
    echo -e "${RED}❌ 错误: M1 版本不存在: $M1_DMG${NC}"
    echo "请先运行: pnpm tauri:build:m1"
    exit 1
fi

echo -e "${GREEN}✅ 找到构建文件:${NC}"
echo "  📁 Intel: $INTEL_DMG"
echo "  📁 M1: $M1_DMG"
echo ""

# 检查 Release 是否存在
echo -e "${YELLOW}🔍 检查 Release ${TAG} 是否存在...${NC}"
if ! gh release view "$TAG" --repo "$REPO" &> /dev/null; then
    echo -e "${RED}❌ 错误: Release ${TAG} 不存在${NC}"
    echo "请先在 GitHub 上创建 Release: https://github.com/${REPO}/releases"
    exit 1
fi

echo -e "${GREEN}✅ Release ${TAG} 存在${NC}"
echo ""

# 上传文件
echo -e "${GREEN}📤 开始上传文件...${NC}"
echo ""

echo -e "${YELLOW}上传 Intel 版本...${NC}"
if gh release upload "$TAG" "$INTEL_DMG" --repo "$REPO" --clobber; then
    echo -e "${GREEN}✅ Intel 版本上传成功${NC}"
else
    echo -e "${RED}❌ Intel 版本上传失败${NC}"
    exit 1
fi
echo ""

echo -e "${YELLOW}上传 M1 版本...${NC}"
if gh release upload "$TAG" "$M1_DMG" --repo "$REPO" --clobber; then
    echo -e "${GREEN}✅ M1 版本上传成功${NC}"
else
    echo -e "${RED}❌ M1 版本上传失败${NC}"
    exit 1
fi
echo ""

echo -e "${GREEN}🎉 所有文件上传完成！${NC}"
echo -e "${GREEN}查看 Release: https://github.com/${REPO}/releases/tag/${TAG}${NC}"

