@echo off
chcp 65001 > nul
echo 🏦 银行卡自动填写测试工具
echo ==================================

REM 检查Python环境
python --version >nul 2>&1
if errorlevel 1 (
    echo ❌ 错误: 未找到 python
    pause
    exit /b 1
)

REM 进入脚本目录
cd /d "%~dp0\src-tauri\python_scripts"

REM 检查虚拟环境
if exist "venv" (
    echo 📝 激活虚拟环境...
    call venv\Scripts\activate.bat
) else (
    echo ⚠️ 警告: 未找到虚拟环境，使用系统Python
)

REM 检查依赖
echo 📦 检查依赖...
python -c "import DrissionPage, colorama" 2>nul
if errorlevel 1 (
    echo ❌ 缺少依赖，请安装:
    echo pip install DrissionPage colorama
    pause
    exit /b 1
)

echo 🚀 启动银行卡测试工具...
echo.

REM 运行测试脚本
python test_bank_card_fill.py

echo.
echo ✅ 测试完成
pause
