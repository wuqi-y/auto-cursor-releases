#!/bin/bash

# Wrapper 脚本用于从 npm scripts 传递版本号
# 从命令行参数或 npm_config 中获取版本号

VERSION=$1

if [ -z "$VERSION" ]; then
    echo "❌ 错误: 缺少版本号参数"
    echo "使用方法: pnpm upload:release <version>"
    echo "示例: pnpm upload:release 0.4.0"
    exit 1
fi

# 调用实际的上传脚本
bash "$(dirname "$0")/upload-to-release.sh" "$VERSION"

