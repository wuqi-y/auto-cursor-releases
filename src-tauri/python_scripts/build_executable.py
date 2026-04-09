#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
打包Python项目为可执行文件
支持Windows、macOS、Linux
"""

import os
import sys
import subprocess
import shutil
from pathlib import Path

def get_platform_info():
    """获取平台信息"""
    if sys.platform.startswith('win'):
        return 'windows', '.exe'
    elif sys.platform.startswith('darwin'):
        return 'macos', ''
    elif sys.platform.startswith('linux'):
        return 'linux', ''
    else:
        return 'unknown', ''

def build_executable():
    """使用PyInstaller打包可执行文件"""
    platform, ext = get_platform_info()
    
    print(f"🚀 开始为 {platform} 平台打包可执行文件...")
    
    # 获取当前目录
    current_dir = Path(__file__).parent
    build_dir = current_dir.parent / "pyBuild"
    
    # 确保build目录存在
    build_dir.mkdir(exist_ok=True)

    # 创建平台特定目录，如果存在则清理
    platform_dir = build_dir / platform
    if platform_dir.exists():
        print(f"🧹 清理 {platform} 平台的构建目录...")
        shutil.rmtree(platform_dir)
    platform_dir.mkdir(exist_ok=True)
    
    print(f"📁 构建目录: {platform_dir}")
    
    # 激活虚拟环境并安装PyInstaller
    venv_python = current_dir / "venv" / "bin" / "python"
    if platform == 'windows':
        venv_python = current_dir / "venv" / "Scripts" / "python.exe"
    
    if not venv_python.exists():
        print("❌ 虚拟环境不存在，请先创建虚拟环境并安装依赖")
        return False
    
    # 安装PyInstaller
    print("📦 安装PyInstaller...")
    result = subprocess.run([
        str(venv_python), "-m", "pip", "install", "pyinstaller"
    ], capture_output=True, text=True)
    
    if result.returncode != 0:
        print(f"❌ 安装PyInstaller失败: {result.stderr}")
        return False
    
    # 创建入口脚本
    entry_script = current_dir / "cursor_register_entry.py"
    entry_content = '''#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Cursor注册程序入口点
"""

import sys
import json
import os
from pathlib import Path

# 添加当前目录到path
current_dir = Path(__file__).parent
sys.path.insert(0, str(current_dir))

# 设置显示环境
os.environ.setdefault('DISPLAY', ':0')

def main():
    """主函数"""
    if len(sys.argv) < 2:
        print(json.dumps({
            "success": False,
            "error": "缺少参数，用法: cursor_register <email> [first_name] [last_name] [use_incognito] [app_dir] [enable_bank_card_binding] [skip_phone_verification] [config_json]"
        }))
        sys.exit(1)

    email = sys.argv[1]
    first_name = sys.argv[2] if len(sys.argv) > 2 else "Auto"
    last_name = sys.argv[3] if len(sys.argv) > 3 else "Generated"
    use_incognito = sys.argv[4] if len(sys.argv) > 4 else "true"
    app_dir = sys.argv[5] if len(sys.argv) > 5 else None
    enable_bank_card_binding = sys.argv[6] if len(sys.argv) > 6 else "true"
    skip_phone_verification = sys.argv[7] if len(sys.argv) > 7 else "0"
    config_json = sys.argv[8] if len(sys.argv) > 8 else "{}"

    try:
        # 导入manual_register模块并执行
        from manual_register import main as manual_main

        # 临时修改sys.argv来传递参数
        original_argv = sys.argv[:]
        if app_dir is not None:
            sys.argv = ["manual_register.py", email, first_name, last_name, use_incognito, app_dir, enable_bank_card_binding, skip_phone_verification, config_json]
        else:
            sys.argv = ["manual_register.py", email, first_name, last_name, use_incognito, ".", enable_bank_card_binding, skip_phone_verification, config_json]

        try:
            manual_main()
        finally:
            # 恢复原始argv
            sys.argv = original_argv

    except Exception as e:
        print(json.dumps({
            "success": False,
            "error": f"注册过程出错: {str(e)}"
        }, ensure_ascii=False))
        sys.exit(1)

if __name__ == "__main__":
    main()
'''
    
    entry_script.write_text(entry_content, encoding='utf-8')
    
    def run_pyinstaller(target_name: str, script_path: Path, extra_args=None) -> bool:
        """执行单个目标的PyInstaller打包"""
        if extra_args is None:
            extra_args = []

        exe_path = platform_dir / f"{target_name}{ext}"
        work_dir = current_dir / f"build_{target_name}"
        pyinstaller_cmd = [
            str(venv_python), "-m", "PyInstaller",
            "--onefile",  # 单文件模式
            "--console",  # 显示控制台窗口（用于调试）
            "--name", target_name,
            "--distpath", str(platform_dir),
            "--workpath", str(work_dir),
            "--specpath", str(current_dir),
        ] + extra_args + [str(script_path)]

        print(f"🔨 开始打包 {target_name} ...")
        print(f"命令: {' '.join(pyinstaller_cmd)}")

        result = subprocess.run(
            pyinstaller_cmd,
            cwd=str(current_dir),
            capture_output=True,
            text=True
        )

        if result.returncode != 0:
            print(f"❌ {target_name} 打包失败:")
            print(f"stdout: {result.stdout}")
            print(f"stderr: {result.stderr}")
            return False

        if not exe_path.exists():
            print(f"❌ 可执行文件未生成: {exe_path}")
            return False

        print(f"✅ {target_name} 打包成功!")
        print(f"📦 可执行文件: {exe_path}")
        print(f"📏 文件大小: {exe_path.stat().st_size / 1024 / 1024:.1f} MB")
        return True

    common_collect_args = [
        "--collect-all", "colorama",
        "--collect-all", "requests",
        "--collect-all", "DrissionPage",
    ]

    cursor_register_args = common_collect_args + [
        "--collect-all", "faker",
        "--collect-all", "dotenv",
        # 添加隐藏导入
        "--hidden-import", "manual_register",
        "--hidden-import", "cursor_register_manual",
        "--hidden-import", "new_signup",
        "--hidden-import", "cursor_auth",
        "--hidden-import", "reset_machine_manual",
        "--hidden-import", "get_user_token",
        "--hidden-import", "account_manager",
        "--hidden-import", "config",
        "--hidden-import", "utils",
        "--hidden-import", "email_tabs.email_tab_interface",
        "--hidden-import", "email_tabs.tempmail_plus_tab",
        # 添加第三方库的具体模块
        "--hidden-import", "faker.providers.person",
        "--hidden-import", "faker.providers.internet",
        "--hidden-import", "faker.providers.lorem",
        "--hidden-import", "requests.adapters",
        "--hidden-import", "requests.packages.urllib3",
        "--hidden-import", "urllib3.util.retry",
        "--hidden-import", "urllib3.util.connection",
        # 添加数据文件
        "--add-data", f"{current_dir}/*.py{os.pathsep}.",
        "--add-data", f"{current_dir}/email_tabs{os.pathsep}email_tabs",
    ]

    cdp_flow_runner_args = common_collect_args + [
        "--hidden-import", "new_signup",
        "--hidden-import", "openai_oauth_step1",
        "--hidden-import", "requests.adapters",
        "--hidden-import", "requests.packages.urllib3",
        "--hidden-import", "urllib3.util.retry",
        "--hidden-import", "urllib3.util.connection",
        "--add-data", f"{current_dir}/*.py{os.pathsep}.",
    ]

    cursor_ok = run_pyinstaller("cursor_register", entry_script, cursor_register_args)
    cdp_ok = run_pyinstaller(
        "cdp_flow_runner",
        current_dir / "cdp_flow_runner.py",
        cdp_flow_runner_args
    )

    # 清理临时文件
    cleanup_files = [
        current_dir / "cursor_register.spec",
        current_dir / "cdp_flow_runner.spec",
        current_dir / "build_cursor_register",
        current_dir / "build_cdp_flow_runner",
        entry_script
    ]

    for file_path in cleanup_files:
        if file_path.exists():
            if file_path.is_dir():
                shutil.rmtree(file_path)
            else:
                file_path.unlink()

    return cursor_ok and cdp_ok

def create_readme():
    """创建README文件"""
    platform, ext = get_platform_info()
    build_dir = Path(__file__).parent.parent / "pyBuild"
    
    readme_content = f"""# Cursor自动注册 - 可执行文件

## 📦 打包信息
- 平台: {platform}
- 可执行文件: cursor_register{ext}, cdp_flow_runner{ext}
- 打包时间: {__import__('datetime').datetime.now().strftime('%Y-%m-%d %H:%M:%S')}

## 🚀 使用方法

```bash
# 1) Cursor 注册（默认启用无痕模式和银行卡绑定）
./cursor_register{ext} test@example.com John Smith

# 只提供邮箱（会生成随机姓名）
./cursor_register{ext} test@example.com

# 完整参数用法
./cursor_register{ext} test@example.com John Smith true . true 0 '{{"btnIndex":1}}'

# 启用跳过手机号验证（实验性功能）
./cursor_register{ext} test@example.com John Smith true . true 1 '{{"btnIndex":1}}'

# 注册美国账户（使用按钮索引2）
./cursor_register{ext} test@example.com John Smith true . true 0 '{{"btnIndex":2}}'

# 参数说明:
# 参数1: 邮箱地址 (必需)
# 参数2: 名字 (可选，默认: Auto)
# 参数3: 姓氏 (可选，默认: Generated)
# 参数4: 无痕模式 (可选，默认: true)
# 参数5: 应用目录 (可选，默认: .)
# 参数6: 银行卡绑定 (可选，默认: true)
# 参数7: 跳过手机号验证 (可选，默认: 0，设置为1启用实验性功能)
# 参数8: 配置JSON (可选，默认: {{}})
#        - btnIndex: 按钮索引，1=默认地区，2=美国账户

# 2) CDP 流程运行器（与你在 python 里调用 cdp_flow_runner.py 参数一致）
./cdp_flow_runner{ext} --url "https://chatgpt.com/" --click "css:button[class*='btn-secondary']"
```

## 📊 响应格式

成功:
```json
{{"success": true, "email": "test@example.com", "message": "注册成功"}}
```

失败:
```json
{{"success": false, "error": "错误信息"}}
```

## ⚠️ 注意事项

1. 需要Chrome/Chromium浏览器
2. 需要稳定的网络连接
3. 首次运行可能需要较长时间加载
"""
    
    readme_path = build_dir / platform / "README.md"
    readme_path.write_text(readme_content, encoding='utf-8')
    print(f"📝 README已创建: {readme_path}")

if __name__ == "__main__":
    if build_executable():
        create_readme()
        print("🎉 打包完成!")
    else:
        print("❌ 打包失败!")
        sys.exit(1)
