#!/bin/bash

# 银行卡自动填写测试脚本启动器

echo "🏦 银行卡自动填写测试工具"
echo "=================================="

# 检查Python环境
if ! command -v python3 &> /dev/null; then
    echo "❌ 错误: 未找到 python3"
    exit 1
fi

# 进入脚本目录
cd "$(dirname "$0")/src-tauri/python_scripts"

# 检查虚拟环境
if [ -d "venv" ]; then
    echo "📝 激活虚拟环境..."
    source venv/bin/activate
else
    echo "⚠️ 警告: 未找到虚拟环境，使用系统Python"
fi

# 检查依赖
echo "📦 检查依赖..."
python3 -c "import DrissionPage, colorama" 2>/dev/null
if [ $? -ne 0 ]; then
    echo "❌ 缺少依赖，请安装:"
    echo "pip install DrissionPage colorama"
    exit 1
fi

echo "🚀 启动银行卡测试工具..."
echo ""

# 运行测试脚本
python3 test_bank_card_fill.py

echo ""
echo "✅ 测试完成"
