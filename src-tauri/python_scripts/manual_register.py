#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
手动注册脚本 - 简化版本，不依赖翻译器
"""

import os
import sys
import io
import json
import base64
from pathlib import Path
from faker import Faker

# 设置UTF-8编码输出 - 必须在所有其他操作之前
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8')
sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding='utf-8')

# 强制刷新输出，确保实时显示
sys.stdout.reconfigure(line_buffering=True)
sys.stderr.reconfigure(line_buffering=True)

def safe_print(*args, **kwargs):
    """Safe print function that handles BrokenPipeError"""
    try:
        print(*args, **kwargs)
        sys.stdout.flush()
    except BrokenPipeError:
        # Pipe has been closed, exit gracefully
        sys.exit(0)
    except Exception:
        # Ignore other print errors
        pass

# 设置路径
current_dir = Path(__file__).parent
sys.path.insert(0, str(current_dir))

# 设置显示环境
os.environ.setdefault('DISPLAY', ':0')

class SimpleTranslator:
    """简单的翻译器替代品，返回中文文本"""

    def get(self, key, **kwargs):
        translations = {
            'register.password': '密码',
            'register.first_name': '名字',
            'register.last_name': '姓氏',
            'register.suggest_email': f'建议邮箱: {kwargs.get("suggested_email", "")}',
            'register.use_suggested_email_or_enter': '输入 "yes" 使用建议邮箱或输入您自己的邮箱:',
            'register.manual_email_input': '请输入您的邮箱地址:',
            'register.invalid_email': '无效的邮箱地址',
            'register.email_address': '邮箱地址',
            'register.email_setup_failed': f'邮箱设置失败: {kwargs.get("error", "")}',
            'register.manual_code_input': '请输入验证码:',
            'register.invalid_code': '无效的验证码',
            'register.code_input_failed': f'验证码输入失败: {kwargs.get("error", "")}',
            'register.register_start': '开始注册',
            'register.using_tempmail_plus': '使用 TempMail Plus',
            'register.register_process_error': f'注册过程出错: {kwargs.get("error", "")}',
            'register.get_token': '获取令牌',
            'register.token_success': '令牌获取成功',
            'register.token_attempt': f'第 {kwargs.get("attempt", 0)} 次尝试，等待 {kwargs.get("time", 0)} 秒',
            'register.token_max_attempts': f'超过最大尝试次数 {kwargs.get("max", 0)}',
            'register.token_failed': f'令牌获取失败: {kwargs.get("error", "")}',
            'register.account_error': f'账户信息获取失败: {kwargs.get("error", "")}',
            'register.update_cursor_auth_info': '更新 Cursor 认证信息',
            'register.cursor_auth_info_updated': 'Cursor 认证信息已更新',
            'register.cursor_auth_info_update_failed': 'Cursor 认证信息更新失败',
            'register.reset_machine_id': '重置机器ID',
            'register.save_account_info_failed': f'保存账户信息失败: {kwargs.get("error", "")}',
            'register.cursor_registration_completed': 'Cursor 注册完成',
            'register.title': 'Cursor 手动注册',
            'register.press_enter': '按回车键继续...',
            # 添加更多翻译
            'register.using_browser': f'使用浏览器: {kwargs.get("browser", "")} 路径: {kwargs.get("path", "")}',
            'register.starting_browser': '启动浏览器',
            'register.browser_started': '浏览器已启动',
            'register.visiting_url': '访问网址',
            'register.waiting_for_page_load': '等待页面加载',
            'register.filling_form': '填写注册表单',
            'register.form_success': '表单填写成功',
            'register.form_error': f'表单填写失败: {kwargs.get("error", "")}',
            'register.form_submitted': '表单已提交',
            'register.first_verification_passed': '第一次验证通过',
            'register.waiting_for_second_verification': '等待第二次验证',
            'register.second_verification_failed': '第二次验证失败',
            'register.first_verification_failed': '第一次验证失败',
            'register.tracking_processes': f'跟踪 {kwargs.get("count", 0)} 个 {kwargs.get("browser", "")} 进程',
            'register.no_new_processes_detected': f'未检测到新的 {kwargs.get("browser", "")} 进程',
            'register.could_not_track_processes': f'无法跟踪 {kwargs.get("browser", "")} 进程: {kwargs.get("error", "")}',
            'register.browser_setup_error': f'浏览器设置错误: {kwargs.get("error", "")}',
            'register.handling_turnstile': '处理 Turnstile 验证',
            'register.retry_verification': f'第 {kwargs.get("attempt", 0)} 次验证尝试',
            'register.detect_turnstile': '检测到验证框',
            'register.verification_success': '验证成功',
            'register.verification_failed': '验证失败',
            'register.verification_error': f'验证过程出错: {kwargs.get("error", "")}',
            'register.waiting_for_verification_code': '等待验证码',
            'register.verification_code_processing_failed': '验证码处理失败',
        }
        return translations.get(key, key)

def main():
    """主函数"""
    if len(sys.argv) < 2:
        print(json.dumps({
            "success": False,
            "error": "缺少参数，用法: python manual_register.py <email> [first_name] [last_name] [use_incognito] [app_dir]"
        }, ensure_ascii=False))
        sys.exit(1)
    
    email = sys.argv[1]
    app_dir = None
    
    # 解析参数：email first_name last_name [use_incognito] [app_dir] [enable_bank_card_binding] [skip_phone_verification] [config_json]
    enable_bank_card_binding = True  # 默认值
    skip_phone_verification = False  # 默认值
    config_dict = {}  # 默认空配置
    
    if len(sys.argv) >= 9:
        # 有9个或更多参数：包含所有参数 + 配置JSON
        first_name = sys.argv[2]
        last_name = sys.argv[3]
        use_incognito = sys.argv[4]
        app_dir_base64 = sys.argv[5]
        enable_bank_card_binding_str = sys.argv[6]
        skip_phone_verification_str = sys.argv[7]
        config_json_str = sys.argv[8]
        
        enable_bank_card_binding = enable_bank_card_binding_str.lower() == "true"
        skip_phone_verification = skip_phone_verification_str == "1"
        
        # 解析配置JSON
        try:
            config_dict = json.loads(config_json_str)
            print(f"🔍 [DEBUG] 配置JSON解析成功: {config_dict}")
        except Exception as e:
            print(f"🔍 [DEBUG] 配置JSON解析失败: {str(e)}, 使用默认配置")
            config_dict = {}
        
        # 解码 Base64 编码的应用目录
        try:
            app_dir = base64.b64decode(app_dir_base64).decode('utf-8')
            print(f"🔍 [DEBUG] Base64解码成功: {app_dir_base64} -> {app_dir}")
        except Exception as e:
            print(f"🔍 [DEBUG] Base64解码失败: {str(e)}, 直接使用原始值")
            app_dir = app_dir_base64
    elif len(sys.argv) >= 8:
        # 有8个或更多参数：包含银行卡绑定参数和跳过手机号验证参数
        first_name = sys.argv[2]
        last_name = sys.argv[3]
        use_incognito = sys.argv[4]
        app_dir_base64 = sys.argv[5]
        enable_bank_card_binding_str = sys.argv[6]
        skip_phone_verification_str = sys.argv[7]
        enable_bank_card_binding = enable_bank_card_binding_str.lower() == "true"
        skip_phone_verification = skip_phone_verification_str == "1"
        
        # 解码 Base64 编码的应用目录
        try:
            app_dir = base64.b64decode(app_dir_base64).decode('utf-8')
            print(f"🔍 [DEBUG] Base64解码成功: {app_dir_base64} -> {app_dir}")
        except Exception as e:
            print(f"🔍 [DEBUG] Base64解码失败: {str(e)}, 直接使用原始值")
            app_dir = app_dir_base64
    elif len(sys.argv) >= 7:
        # 有7个参数：包含银行卡绑定参数（向后兼容，没有跳过手机号验证）
        first_name = sys.argv[2]
        last_name = sys.argv[3]
        use_incognito = sys.argv[4]
        app_dir_base64 = sys.argv[5]
        enable_bank_card_binding_str = sys.argv[6]
        enable_bank_card_binding = enable_bank_card_binding_str.lower() == "true"
        
        # 解码 Base64 编码的应用目录
        try:
            app_dir = base64.b64decode(app_dir_base64).decode('utf-8')
            print(f"🔍 [DEBUG] Base64解码成功: {app_dir_base64} -> {app_dir}")
        except Exception as e:
            print(f"🔍 [DEBUG] Base64解码失败: {str(e)}, 直接使用原始值")
            app_dir = app_dir_base64
    elif len(sys.argv) >= 6:
        # 有6个参数：包含应用目录（Base64编码），但没有银行卡参数（向后兼容）
        first_name = sys.argv[2]
        last_name = sys.argv[3]
        use_incognito = sys.argv[4]
        app_dir_base64 = sys.argv[5]
        
        # 解码 Base64 编码的应用目录
        try:
            app_dir = base64.b64decode(app_dir_base64).decode('utf-8')
            print(f"🔍 [DEBUG] Base64解码成功: {app_dir_base64} -> {app_dir}")
        except Exception as e:
            print(f"🔍 [DEBUG] Base64解码失败: {str(e)}, 直接使用原始值")
            app_dir = app_dir_base64
    elif len(sys.argv) >= 5:
        # 有5个参数：包含无痕模式设置，但没有应用目录
        first_name = sys.argv[2]
        last_name = sys.argv[3]
        use_incognito = sys.argv[4]
    elif len(sys.argv) >= 4:
        # 有4个参数：没有无痕模式设置，使用默认值
        first_name = sys.argv[2]
        last_name = sys.argv[3]
        use_incognito = "true"
    elif len(sys.argv) == 3:
        first_name = sys.argv[2]
        faker = Faker()
        last_name = faker.last_name()
        use_incognito = "true"
    else:
        faker = Faker()
        first_name = faker.first_name()
        last_name = faker.last_name()
        use_incognito = "true"

    # 转换无痕模式参数
    use_incognito_bool = use_incognito.lower() == "true"

    # 调试日志
    print(f"🔍 [DEBUG] 接收到的参数:")
    print(f"  - 邮箱: {email}")
    print(f"  - 姓名: {first_name} {last_name}")
    print(f"  - 无痕模式参数: {use_incognito}")
    print(f"  - 无痕模式布尔值: {use_incognito_bool}")
    print(f"  - 应用目录: {app_dir}")
    print(f"  - 银行卡绑定: {enable_bank_card_binding}")
    print(f"  - 跳过手机号验证: {skip_phone_verification}")
    print(f"  - 配置参数: {config_dict}")
    print(f"  - 总参数数量: {len(sys.argv)}")
    print(f"  - 所有参数: {sys.argv}")
    print(f"🔍 [DEBUG] 详细参数解析:")
    for i, arg in enumerate(sys.argv):
        print(f"  - sys.argv[{i}]: '{arg}' (类型: {type(arg)}, 长度: {len(arg)})")
    
    try:
        print(f"🎯 开始注册 Cursor 账户")
        print(f"📧 邮箱: {email}")
        print(f"👤 姓名: {first_name} {last_name}")
        
        # 导入必要的模块
        from cursor_register_manual import CursorRegistration
        from new_signup import cleanup_chrome_processes
        
        # 创建简单翻译器
        translator = SimpleTranslator()
        
        # 创建注册实例
        registration = CursorRegistration(translator=translator, use_incognito=use_incognito_bool, app_dir=app_dir, enable_bank_card_binding=enable_bank_card_binding, skip_phone_verification=skip_phone_verification, config=config_dict)

        # 设置用户信息
        registration.email_address = email
        registration.first_name = first_name
        registration.last_name = last_name

        # 显示最终使用的信息
        print(f"\n🔑 使用密码: {registration.password}", flush=True)
        print(f"👤 使用姓名: {first_name} {last_name}", flush=True)
        print(f"📧 使用邮箱: {email}", flush=True)

        # 显示姓名信息（用于调试）
        if translator:
            print(f"{translator.get('register.first_name')}: {first_name}", flush=True)
            print(f"{translator.get('register.last_name')}: {last_name}", flush=True)
        else:
            print(f"名字: {first_name}", flush=True)
            print(f"姓氏: {last_name}", flush=True)

        # 执行注册流程
        success = False
        try:
            # 执行注册，支持前端验证码输入
            if registration.register_cursor():
                success = True

                # 准备返回的数据
                result_data = {
                    "success": True,
                    "email": email,
                    "first_name": first_name,
                    "last_name": last_name,
                    "message": "注册成功",
                    "status": "completed"
                }

                # 添加token信息（如果可用）
                if hasattr(registration, 'extracted_token') and registration.extracted_token:
                    result_data["token"] = registration.extracted_token

                if hasattr(registration, 'workos_cursor_session_token') and registration.workos_cursor_session_token:
                    result_data["workos_cursor_session_token"] = registration.workos_cursor_session_token

                print(json.dumps(result_data, ensure_ascii=False), flush=True)
            else:
                safe_print(json.dumps({
                    "success": False,
                    "error": "注册失败",
                    "message": "注册过程中出现错误"
                }, ensure_ascii=False))
        except Exception as e:
            safe_print(json.dumps({
                "success": False,
                "error": f"注册过程出错: {str(e)}"
            }, ensure_ascii=False))
                
    except ImportError as e:
        safe_print(json.dumps({
            "success": False,
            "error": f"导入模块失败: {str(e)}"
        }, ensure_ascii=False))
        sys.exit(1)
    except Exception as e:
        safe_print(json.dumps({
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
