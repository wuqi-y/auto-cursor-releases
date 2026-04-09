#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
独立的Cursor注册入口 - 包含所有必要的代码
"""

import os
import sys
import json
from pathlib import Path

# 设置路径
current_dir = Path(__file__).parent
sys.path.insert(0, str(current_dir))

# 设置显示环境
os.environ.setdefault('DISPLAY', ':0')

def main():
    """主函数 - 直接调用原始的cursor_register_manual流程"""
    if len(sys.argv) < 2:
        print(json.dumps({
            "success": False,
            "error": "缺少参数，用法: cursor_register <email> [first_name] [last_name]"
        }))
        sys.exit(1)
    
    email = sys.argv[1]
    first_name = sys.argv[2] if len(sys.argv) > 2 else "Auto"
    last_name = sys.argv[3] if len(sys.argv) > 3 else "Generated"
    
    try:
        print(f"🎯 开始注册 Cursor 账户")
        print(f"📧 邮箱: {email}")
        print(f"👤 姓名: {first_name} {last_name}")
        
        # 检查PyInstaller运行时路径
        if hasattr(sys, '_MEIPASS'):
            # PyInstaller runtime
            base_path = Path(sys._MEIPASS)
        else:
            # Development
            base_path = current_dir
            
        sys.path.insert(0, str(base_path))
        
        try:
            # 导入必要的模块
            from cursor_register_manual import CursorRegistration
            from new_signup import cleanup_chrome_processes
            
            # 创建注册实例
            registration = CursorRegistration(translator=None)
            
            # 设置用户信息
            registration.email_address = email
            registration.first_name = first_name  
            registration.last_name = last_name
            
            # 执行注册流程
            success = False
            if registration.setup_email():
                success = registration.register_cursor()
            
            if success:
                print(json.dumps({
                    "success": True,
                    "email": email,
                    "first_name": first_name,
                    "last_name": last_name,
                    "message": "注册成功"
                }, ensure_ascii=False))
            else:
                print(json.dumps({
                    "success": False,
                    "error": "注册失败"
                }, ensure_ascii=False))
                
        except ImportError as e:
            print(json.dumps({
                "success": False,
                "error": f"导入模块失败: {str(e)}"
            }))
            # 打印调试信息
            print(f"Base path: {base_path}")
            print(f"Sys path: {sys.path}")
            print(f"Available files: {list(base_path.glob('*.py')) if base_path.exists() else 'Path does not exist'}")
            sys.exit(1)
            
    except Exception as e:
        print(json.dumps({
            "success": False,
            "error": f"注册过程出错: {str(e)}"
        }, ensure_ascii=False))
    finally:
        # 清理Chrome进程
        try:
            cleanup_chrome_processes()
        except:
            pass

if __name__ == "__main__":
    main()
