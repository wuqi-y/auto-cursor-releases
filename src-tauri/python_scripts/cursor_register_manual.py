import os
import sys
import json
from colorama import Fore, Style, init
import time
import random
from faker import Faker

# 强制刷新输出，确保实时显示
sys.stdout.reconfigure(line_buffering=True)
sys.stderr.reconfigure(line_buffering=True)
from cursor_auth import CursorAuth
from reset_machine_manual import MachineIDResetter
from get_user_token import get_token_from_cookie
from config import get_config
from account_manager import AccountManager
from new_signup import wait_for_phone_verification

os.environ["PYTHONVERBOSE"] = "0"
os.environ["PYINSTALLER_VERBOSE"] = "0"

# Initialize colorama
init()

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

# Define emoji constants
EMOJI = {
    'START': '🚀',
    'FORM': '📝',
    'VERIFY': '🔄',
    'PASSWORD': '🔑',
    'CODE': '📱',
    'DONE': '✨',
    'ERROR': '❌',
    'WAIT': '⏳',
    'SUCCESS': '✅',
    'MAIL': '📧',
    'KEY': '🔐',
    'UPDATE': '🔄',
    'INFO': 'ℹ️',
    'WARNING': '⚠️'
}

def get_random_wait_time(config, timing_type='page_load_wait'):
    """
    Get random wait time from config
    Args:
        config: ConfigParser object
        timing_type: Type of timing to get (page_load_wait, input_wait, submit_wait)
    Returns:
        float: Random wait time or fixed time
    """
    try:
        if not config.has_section('Timing'):
            return random.uniform(0.1, 0.8)  # Default value

        if timing_type == 'random':
            min_time = float(config.get('Timing', 'min_random_time', fallback='0.1'))
            max_time = float(config.get('Timing', 'max_random_time', fallback='0.8'))
            return random.uniform(min_time, max_time)

        time_value = config.get('Timing', timing_type, fallback='0.1-0.8')

        # Check if it's a fixed time value
        if '-' not in time_value and ',' not in time_value:
            return float(time_value)  # Return fixed time

        # Process range time
        min_time, max_time = map(float, time_value.split('-' if '-' in time_value else ','))
        return random.uniform(min_time, max_time)
    except:
        return random.uniform(0.1, 0.8)  # Return default value when error

class CursorRegistration:
    def __init__(self, translator=None, use_incognito=True, app_dir=None, enable_bank_card_binding=True, skip_phone_verification=False, config=None):
        self.translator = translator
        # Set to display mode
        os.environ['BROWSER_HEADLESS'] = 'False'
        self.browser = None
        self.controller = None
        self.sign_up_url = "https://authenticator.cursor.sh/sign-up"
        self.settings_url = "https://www.cursor.com/settings"
        self.email_address = None
        self.signup_tab = None
        self.email_tab = None
        self.use_incognito = use_incognito  # 无痕模式设置
        self.app_dir = app_dir  # 应用目录路径
        self.keep_browser_open = False  # 标记是否保持浏览器打开
        self.enable_bank_card_binding = enable_bank_card_binding  # 是否启用银行卡绑定
        self.custom_config = config or {}  # 自定义配置参数（JSON字典）
        self.skip_phone_verification = skip_phone_verification  # 是否跳过手机号验证

        # 获取默认配置
        self.config = get_config(translator)

        # 调试日志
        print(f"🔍 [DEBUG] CursorRegistration 初始化:")
        print(f"  - 无痕模式设置: {self.use_incognito}")
        print(f"  - 应用目录: {self.app_dir}")
        print(f"  - 银行卡绑定设置: {self.enable_bank_card_binding}")
        print(f"  - 跳过手机号验证: {self.skip_phone_verification}")

        # initialize Faker instance
        self.faker = Faker()

        # Token information
        self.extracted_token = None
        self.workos_cursor_session_token = None

        # generate account information
        self.password = self._generate_password()
        # 不在构造函数中生成姓名，等待外部设置
        self.first_name = None
        self.last_name = None

        print(f"\n{Fore.CYAN}{EMOJI['PASSWORD']} {self.translator.get('register.password') if self.translator else '密码'}: {self.password} {Style.RESET_ALL}")

    def _generate_password(self, length=12):
        """Generate password"""
        return self.faker.password(length=length, special_chars=True, digits=True, upper_case=True, lower_case=True)

    def setup_email(self):
        """Setup Email"""
        try:
            # Try to get a suggested email
            account_manager = AccountManager(self.translator)
            suggested_email = account_manager.suggest_email(self.first_name, self.last_name)
            
            if suggested_email:
                if self.translator:
                    print(f"{Fore.CYAN}{EMOJI['START']} {self.translator.get('register.suggest_email', suggested_email=suggested_email)}")
                else:
                    print(f"{Fore.CYAN}{EMOJI['START']} Suggested email: {suggested_email}")
                if self.translator:
                    print(f"{Fore.CYAN}{EMOJI['START']} {self.translator.get('register.use_suggested_email_or_enter')}")
                else:
                    print(f"{Fore.CYAN}{EMOJI['START']} Type 'yes' to use this email or enter your own email:")
                user_input = input().strip()
                
                if user_input.lower() == 'yes' or user_input.lower() == 'y':
                    self.email_address = suggested_email
                else:
                    # User input is their own email address
                    self.email_address = user_input
            else:
                # If there's no suggested email
                print(f"{Fore.CYAN}{EMOJI['START']} {self.translator.get('register.manual_email_input') if self.translator else 'Please enter your email address:'}")
                self.email_address = input().strip()
            
            # Validate if the email is valid
            if '@' not in self.email_address:
                print(f"{Fore.RED}{EMOJI['ERROR']} {self.translator.get('register.invalid_email') if self.translator else 'Invalid email address'}{Style.RESET_ALL}")
                return False
                
            print(f"{Fore.CYAN}{EMOJI['MAIL']} {self.translator.get('register.email_address')}: {self.email_address}" + "\n" + f"{Style.RESET_ALL}")
            return True
            
        except Exception as e:
            print(f"{Fore.RED}{EMOJI['ERROR']} {self.translator.get('register.email_setup_failed', error=str(e))}{Style.RESET_ALL}")
            return False

    def get_verification_code(self):
        """Get Verification Code from frontend via temp file"""
        import tempfile
        import os

        try:
            # 输出JSON格式的请求，让前端知道需要验证码
            print(json.dumps({
                "action": "request_verification_code",
                "message": "请输入6位验证码",
                "status": "waiting_for_code"
            }, ensure_ascii=False))
            print(f"{Fore.CYAN}{EMOJI['CODE']} 等待前端输入验证码...{Style.RESET_ALL}")

            # 等待前端通过临时文件传递验证码
            # 优先使用环境变量中指定的验证码文件路径（用于并行注册隔离）
            # 如果环境变量不存在，则使用默认路径（向后兼容）
            temp_dir = tempfile.gettempdir()
            code_file = os.environ.get('CURSOR_VERIFICATION_CODE_FILE', 
                                       os.path.join(temp_dir, "cursor_verification_code.txt"))
            cancel_file = os.path.join(temp_dir, "cursor_registration_cancel.txt")

            print(f"{Fore.CYAN}{EMOJI['INFO']} 临时目录: {temp_dir}{Style.RESET_ALL}")
            print(f"{Fore.CYAN}{EMOJI['INFO']} 验证码文件: {code_file}{Style.RESET_ALL}")
            print(f"{Fore.CYAN}{EMOJI['INFO']} 取消文件: {cancel_file}{Style.RESET_ALL}")

            # 清理可能存在的旧文件
            for file_path in [code_file, cancel_file]:
                if os.path.exists(file_path):
                    try:
                        os.remove(file_path)
                        print(f"{Fore.YELLOW}{EMOJI['INFO']} 清理旧文件: {file_path}{Style.RESET_ALL}")
                    except Exception as e:
                        print(f"{Fore.RED}{EMOJI['ERROR']} 清理文件失败 {file_path}: {e}{Style.RESET_ALL}")

            # 第一阶段：自动获取验证码，等待30秒
            initial_wait = 30
            wait_time = 0

            print(f"{Fore.CYAN}{EMOJI['INFO']} 尝试自动获取验证码 (最多等待 {initial_wait} 秒)...{Style.RESET_ALL}")

            # 30秒内尝试自动获取
            while wait_time < initial_wait:
                # 检查是否有取消请求
                if os.path.exists(cancel_file):
                    print(f"{Fore.YELLOW}{EMOJI['INFO']} 收到取消请求，停止等待验证码{Style.RESET_ALL}")
                    try:
                        os.remove(cancel_file)
                    except:
                        pass
                    return None

                if os.path.exists(code_file):
                    try:
                        with open(code_file, 'r') as f:
                            code = f.read().strip()

                        # 删除临时文件
                        try:
                            os.remove(code_file)
                        except:
                            pass

                        # 验证验证码格式
                        if code.isdigit() and len(code) == 6:
                            print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 自动获取验证码成功: {code}{Style.RESET_ALL}")
                            return code
                        elif code.lower() == 'cancel':
                            print(f"{Fore.YELLOW}{EMOJI['INFO']} 用户取消验证码输入{Style.RESET_ALL}")
                            return None

                    except Exception as e:
                        print(f"{Fore.RED}{EMOJI['ERROR']} 读取验证码文件失败: {str(e)}{Style.RESET_ALL}")

                # 每10秒显示一次等待状态
                if wait_time % 10 == 0 and wait_time > 0:
                    remaining = initial_wait - wait_time
                    print(f"{Fore.YELLOW}{EMOJI['INFO']} 等待自动获取验证码... (剩余 {remaining} 秒){Style.RESET_ALL}")

                time.sleep(1)
                wait_time += 1

            # 30秒超时后，通知前端弹出手动输入框
            print(f"{Fore.YELLOW}{EMOJI['WARNING']} 自动获取验证码超时 ({initial_wait}秒){Style.RESET_ALL}")
            print(f"{Fore.CYAN}{EMOJI['INFO']} 等待用户手动输入验证码...{Style.RESET_ALL}")
            
            # 输出JSON格式通知前端超时，需要弹出手动输入框
            print(json.dumps({
                "action": "verification_timeout",
                "message": "自动获取验证码超时，请手动输入验证码",
                "status": "manual_input_required"
            }, ensure_ascii=False))
            
            # 第二阶段：无限等待用户手动输入
            print(f"{Fore.YELLOW}{EMOJI['INFO']} 程序将等待用户手动输入验证码...{Style.RESET_ALL}")
            manual_wait_time = 0
            
            while True:
                # 检查是否有取消请求
                if os.path.exists(cancel_file):
                    print(f"{Fore.YELLOW}{EMOJI['INFO']} 收到取消请求，停止等待验证码{Style.RESET_ALL}")
                    try:
                        os.remove(cancel_file)
                    except:
                        pass
                    return None

                if os.path.exists(code_file):
                    try:
                        with open(code_file, 'r') as f:
                            code = f.read().strip()

                        # 删除临时文件
                        try:
                            os.remove(code_file)
                        except:
                            pass

                        # 验证验证码格式
                        if code.isdigit() and len(code) == 6:
                            print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 收到手动输入的验证码: {code}{Style.RESET_ALL}")
                            return code
                        elif code.lower() == 'cancel':
                            print(f"{Fore.YELLOW}{EMOJI['INFO']} 用户取消验证码输入{Style.RESET_ALL}")
                            return None
                        else:
                            print(f"{Fore.RED}{EMOJI['ERROR']} 无效的验证码格式: {code}，请重新输入{Style.RESET_ALL}")
                            # 清理错误的文件
                            if os.path.exists(code_file):
                                try:
                                    os.remove(code_file)
                                except:
                                    pass

                    except Exception as e:
                        print(f"{Fore.RED}{EMOJI['ERROR']} 读取验证码文件失败: {str(e)}{Style.RESET_ALL}")

                # 每30秒提示一次还在等待
                if manual_wait_time % 30 == 0 and manual_wait_time > 0:
                    print(f"{Fore.CYAN}{EMOJI['INFO']} 仍在等待手动输入验证码... (已等待 {manual_wait_time} 秒){Style.RESET_ALL}")

                time.sleep(1)
                manual_wait_time += 1

        except Exception as e:
            print(f"{Fore.RED}{EMOJI['ERROR']} {self.translator.get('register.code_input_failed', error=str(e)) if self.translator else f'验证码输入失败: {str(e)}'}{Style.RESET_ALL}")
            return None

    def register_cursor(self):
        """Register Cursor"""
        browser_tab = None
        try:
            print(f"{Fore.CYAN}{EMOJI['START']} {self.translator.get('register.register_start')}...{Style.RESET_ALL}")
            
            # Check if tempmail_plus is enabled
            config = get_config(self.translator)
            email_tab = None
            if config and config.has_section('TempMailPlus'):
                if config.getboolean('TempMailPlus', 'enabled'):
                    email = config.get('TempMailPlus', 'email')
                    epin = config.get('TempMailPlus', 'epin')
                    if email and epin:
                        from email_tabs.tempmail_plus_tab import TempMailPlusTab
                        email_tab = TempMailPlusTab(email, epin, self.translator)
                        print(f"{Fore.CYAN}{EMOJI['MAIL']} {self.translator.get('register.using_tempmail_plus')}{Style.RESET_ALL}")
            
            # Use new_signup.py directly for registration
            from new_signup import main as new_signup_main
            
            # Execute new registration process, passing translator
            result, browser_tab = new_signup_main(
                email=self.email_address,
                password=self.password,
                first_name=self.first_name,
                last_name=self.last_name,
                email_tab=email_tab,  # Pass email_tab if tempmail_plus is enabled
                controller=self,  # Pass self instead of self.controller
                translator=self.translator,
                use_incognito=self.use_incognito,  # Pass incognito mode setting
                skip_phone_verification=self.skip_phone_verification,  # Pass skip phone verification setting
                custom_config=self.custom_config  # Pass custom config for proxy settings
                # app_dir is not passed to new_signup_main, it's only used in this class
            )
            
            if result:
                # Use the returned browser instance to get account information
                self.signup_tab = browser_tab  # Save browser instance
                success = self._get_account_info()

                if success:
                    # 根据配置决定是否执行银行卡绑定流程
                    print(f"{Fore.CYAN}{EMOJI['INFO']} 检查银行卡绑定设置: {self.enable_bank_card_binding}{Style.RESET_ALL}")
                    if self.enable_bank_card_binding:
                        print(f"{Fore.CYAN}{EMOJI['INFO']} 开始银行卡绑定流程...{Style.RESET_ALL}")
                        card_success = self._setup_payment_method(browser_tab)
                        # 不管银行卡绑定成功或失败，都保持浏览器打开
                        print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 银行卡绑定流程已完成{Style.RESET_ALL}")
                        print(f"{Fore.CYAN}{EMOJI['INFO']} 浏览器将保持打开状态，请查看结果{Style.RESET_ALL}")
                        print(f"{Fore.YELLOW}{EMOJI['INFO']} 如果有剩余表单或验证未通过，请手动完成剩余的地址信息填写和表单提交{Style.RESET_ALL}")
                        print(f"{Fore.CYAN}{EMOJI['INFO']} 完成后请手动关闭浏览器或终止程序{Style.RESET_ALL}")
                        # 设置标记，不关闭浏览器，并保持进程运行
                        self.keep_browser_open = True
                        self._wait_for_user_completion(browser_tab)
                        return True
                    else:
                        print(f"{Fore.CYAN}{EMOJI['INFO']} 跳过银行卡绑定流程（已禁用）{Style.RESET_ALL}")
                        print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 注册完成，无需银行卡绑定{Style.RESET_ALL}")
                        print(f"{Fore.CYAN}{EMOJI['INFO']} 浏览器将保持打开状态，请查看结果{Style.RESET_ALL}")
                        print(f"{Fore.YELLOW}{EMOJI['INFO']} 如果有剩余操作，请手动完成{Style.RESET_ALL}")
                        print(f"{Fore.CYAN}{EMOJI['INFO']} 完成后请手动关闭浏览器或终止程序{Style.RESET_ALL}")
                        # 设置标记，不关闭浏览器
                        self.keep_browser_open = True
                        self._wait_for_user_completion(browser_tab)
                        return True
                else:
                    # 注册失败，也保持浏览器打开以便查看问题
                    print(f"{Fore.YELLOW}{EMOJI['WARNING']} 注册过程未完全成功，浏览器将保持打开状态以便查看{Style.RESET_ALL}")
                    print(f"{Fore.YELLOW}{EMOJI['INFO']} 请检查浏览器并手动完成剩余的操作{Style.RESET_ALL}")
                    print(f"{Fore.CYAN}{EMOJI['INFO']} 完成后请手动关闭浏览器或终止程序{Style.RESET_ALL}")
                    self.keep_browser_open = True
                    self._wait_for_user_completion(browser_tab)
                    return True

                # Close browser after getting information (except for non-China addresses)
                if browser_tab and not self.keep_browser_open:
                    try:
                        browser_tab.quit()
                    except:
                        pass

                return success
            
            return False
            
        except Exception as e:
            safe_print(f"{Fore.RED}{EMOJI['ERROR']} {self.translator.get('register.register_process_error', error=str(e))}{Style.RESET_ALL}")
            return False
        finally:
            # Ensure browser is closed in any case (except when keep_browser_open is True)
            if browser_tab and not self.keep_browser_open:
                try:
                    browser_tab.quit()
                except:
                    pass
                
    def _get_account_info(self):
        """Get Account Information and Token"""
        try:
            # 在跳转到 settings 之前，先检测是否需要手机号验证
            print(f"{Fore.CYAN}{EMOJI['INFO']} 准备跳转到 settings，先检测手机号验证...{Style.RESET_ALL}")
            if not wait_for_phone_verification(self.signup_tab, self.translator):
                print(f"{Fore.RED}{EMOJI['ERROR']} 手机号验证检查失败{Style.RESET_ALL}")
                return False
            
            self.signup_tab.get(self.settings_url)
            time.sleep(2)
            
            usage_selector = (
                "css:div.col-span-2 > div > div > div > div > "
                "div:nth-child(1) > div.flex.items-center.justify-between.gap-2 > "
                "span.font-mono.text-sm\\/\\[0\\.875rem\\]"
            )
            usage_ele = self.signup_tab.ele(usage_selector)
            total_usage = "未知"
            if usage_ele:
                total_usage = usage_ele.text.split("/")[-1].strip()

            print(f"Total Usage: {total_usage}\n")
            print(f"{Fore.CYAN}{EMOJI['WAIT']} {self.translator.get('register.get_token')}...{Style.RESET_ALL}")
            max_attempts = 30
            retry_interval = 2
            attempts = 0

            while attempts < max_attempts:
                try:
                    cookies = self.signup_tab.cookies()
                    for cookie in cookies:
                        if cookie.get("name") == "WorkosCursorSessionToken":
                            # 保存原始的WorkosCursorSessionToken
                            original_workos_token = cookie["value"]
                            # 打印
                            self.account_info = {
                                "success": True,
                                "wuqi666": original_workos_token,
                                "title": "wuqi666"
                            }
                            
                            # 输出JSON格式的账户信息供前端捕获
                            import json
                            print(json.dumps(self.account_info))
                            print(f"{Fore.CYAN}{EMOJI['INFO']} 找到WorkosCursorSessionToken，开始提取...{Style.RESET_ALL}")
                            # 提取处理后的token
                            # 如果禁用了银行卡绑定，直接输出成功信息，跳过复杂的token处理
                            if not self.enable_bank_card_binding:
                                print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 注册成功，跳过token提取（银行卡绑定已禁用）{Style.RESET_ALL}")
                                self._save_basic_account_info(original_workos_token, total_usage)
                                return True
                            
                            try:
                                token = get_token_from_cookie(cookie["value"], self.translator)
                                print(f"{Fore.GREEN}{EMOJI['SUCCESS']} Token提取成功{Style.RESET_ALL}")
                                print(f"{Fore.CYAN}{EMOJI['INFO']} 原始WorkosCursorSessionToken: {original_workos_token[:50]}...{Style.RESET_ALL}")
                                print(f"{Fore.CYAN}{EMOJI['INFO']} 开始保存账户信息...{Style.RESET_ALL}")
                                save_result = self._save_account_info(token, total_usage, original_workos_token)
                                print(f"{Fore.CYAN}{EMOJI['INFO']} 保存账户信息结果: {save_result}{Style.RESET_ALL}")
                                return save_result
                            except Exception as token_error:
                                print(f"{Fore.RED}{EMOJI['ERROR']} Token提取失败: {str(token_error)}{Style.RESET_ALL}")
                                # 即使token提取失败，也尝试保存基本信息
                                print(f"{Fore.CYAN}{EMOJI['INFO']} 尝试保存基本账户信息...{Style.RESET_ALL}")
                                self._save_basic_account_info(original_workos_token, total_usage)
                                return True

                    attempts += 1
                    if attempts < max_attempts:
                        print(f"{Fore.YELLOW}{EMOJI['WAIT']} {self.translator.get('register.token_attempt', attempt=attempts, time=retry_interval)}{Style.RESET_ALL}")
                        time.sleep(retry_interval)
                    else:
                        print(f"{Fore.RED}{EMOJI['ERROR']} {self.translator.get('register.token_max_attempts', max=max_attempts)}{Style.RESET_ALL}")

                except Exception as e:
                    safe_print(f"{Fore.RED}{EMOJI['ERROR']} {self.translator.get('register.token_failed', error=str(e))}{Style.RESET_ALL}")
                    attempts += 1
                    if attempts < max_attempts:
                        print(f"{Fore.YELLOW}{EMOJI['WAIT']} {self.translator.get('register.token_attempt', attempt=attempts, time=retry_interval)}{Style.RESET_ALL}")
                        time.sleep(retry_interval)

            return False

        except Exception as e:
            safe_print(f"{Fore.RED}{EMOJI['ERROR']} {self.translator.get('register.account_error', error=str(e))}{Style.RESET_ALL}")
            return False

    def _save_basic_account_info(self, original_workos_token, total_usage):
        """保存基本账户信息（当token提取失败时）"""
        try:
            print(f"{Fore.CYAN}{EMOJI['INFO']} 保存基本账户信息，跳过token处理{Style.RESET_ALL}")
            
            # 保存完整的账户信息供输出使用
            self.account_info = {
                "success": True,
                "email": self.email_address,
                "first_name": getattr(self, 'first_name', 'unknown'),
                "last_name": getattr(self, 'last_name', 'unknown'),
                "message": "注册成功",
                "status": "completed",
                "workos_cursor_session_token": original_workos_token
            }
            
            # 输出JSON格式的账户信息供前端捕获
            import json
            print(json.dumps(self.account_info))
            print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 基本账户信息保存成功{Style.RESET_ALL}")
            return True
            
        except Exception as e:
            print(f"{Fore.RED}{EMOJI['ERROR']} 保存基本账户信息失败: {str(e)}{Style.RESET_ALL}")
            return False

    def _save_account_info(self, token, total_usage, original_workos_token=None):
        """Save Account Information to File"""
        try:
            # 注释掉自动切换账号的逻辑，只保存账户信息
            # # Update authentication information first
            # print(f"{Fore.CYAN}{EMOJI['KEY']} {self.translator.get('register.update_cursor_auth_info')}...{Style.RESET_ALL}")
            # if self.update_cursor_auth(email=self.email_address, access_token=token, refresh_token=token, auth_type="Auth_0"):
            #     print(f"{Fore.GREEN}{EMOJI['SUCCESS']} {self.translator.get('register.cursor_auth_info_updated')}...{Style.RESET_ALL}")
            # else:
            #     print(f"{Fore.RED}{EMOJI['ERROR']} {self.translator.get('register.cursor_auth_info_update_failed')}...{Style.RESET_ALL}")

            # # Reset machine ID
            # print(f"{Fore.CYAN}{EMOJI['UPDATE']} {self.translator.get('register.reset_machine_id')}...{Style.RESET_ALL}")
            # resetter = MachineIDResetter(self.translator)  # Create instance with translator
            # if not resetter.reset_machine_ids():  # Call reset_machine_ids method directly
            #     raise Exception("Failed to reset machine ID")

            safe_print(f"{Fore.CYAN}{EMOJI['INFO']} 注册成功，仅保存账户信息，不自动切换账号{Style.RESET_ALL}")

            # Save account information to file using AccountManager
            account_manager = AccountManager(self.translator, self.app_dir)
            if account_manager.save_account_info(self.email_address, self.password, token, total_usage, original_workos_token):
                # 保存token信息供外部访问
                self.extracted_token = token
                self.workos_cursor_session_token = original_workos_token
                
                # 保存完整的账户信息供输出使用
                self.account_info = {
                    "success": True,
                    "email": self.email_address,
                    "first_name": getattr(self, 'first_name', 'unknown'),
                    "last_name": getattr(self, 'last_name', 'unknown'),
                    "message": "注册成功",
                    "status": "completed",
                    "token": token,
                    "workos_cursor_session_token": original_workos_token
                }
                
                # 输出JSON格式的账户信息供前端捕获
                import json
                print(json.dumps(self.account_info))
                
                return True
            else:
                return False

        except Exception as e:
            safe_print(f"{Fore.RED}{EMOJI['ERROR']} {self.translator.get('register.save_account_info_failed', error=str(e))}{Style.RESET_ALL}")
            return False

    def _setup_payment_method(self, browser_tab):
        """设置银行卡支付方式"""
        try:
            print(f"{Fore.CYAN}{EMOJI['INFO']} 跳转到 dashboard 页面...{Style.RESET_ALL}")

            # 跳转到 dashboard 页面
            # browser_tab.get("https://cursor.com/cn/dashboard")
            time.sleep(get_random_wait_time(self.config, 'page_load_wait'))
            
            # 检查是否使用API获取绑卡链接
            use_api_for_bind_card = self.custom_config.get('useApiForBindCard', 1)
            print(f"{Fore.CYAN}{EMOJI['INFO']} 绑卡方式: {'使用API接口' if use_api_for_bind_card == 1 else '模拟按钮点击'}{Style.RESET_ALL}")
            
            if use_api_for_bind_card == 1:
                # 使用API接口获取绑卡链接
                return self._setup_payment_method_via_api(browser_tab)
            else:
                # 使用原有的按钮点击逻辑
                return self._setup_payment_method_via_button_click(browser_tab)

        except Exception as e:
            print(f"{Fore.YELLOW}{EMOJI['WARNING']} 设置支付方式过程出错: {str(e)}，但继续执行后续步骤...{Style.RESET_ALL}")
            # 即使出错也不返回False，继续执行
            try:
                self._fill_payment_form(browser_tab)
            except Exception as form_error:
                print(f"{Fore.YELLOW}{EMOJI['WARNING']} 尝试填写表单出错: {str(form_error)}，继续执行...{Style.RESET_ALL}")
            return True

    def _setup_payment_method_via_api(self, browser_tab):
        """通过API接口获取绑卡链接"""
        try:
            print(f"{Fore.CYAN}{EMOJI['INFO']} 使用API接口获取绑卡链接...{Style.RESET_ALL}")
            
            # 检查是否出现手机号验证页面
            print(f"{Fore.CYAN}{EMOJI['INFO']} 检查 dashboard 页面是否需要手机号验证...{Style.RESET_ALL}")
            if not wait_for_phone_verification(browser_tab, self.translator):
                print(f"{Fore.RED}{EMOJI['ERROR']} 手机号验证检查失败{Style.RESET_ALL}")
                return False
            
            # 获取WorkOS session token
            workos_token = self._get_workos_session_token(browser_tab)
            if not workos_token:
                print(f"{Fore.RED}{EMOJI['ERROR']} 无法获取WorkOS session token{Style.RESET_ALL}")
                return False
            
            # 从config中获取订阅配置参数
            subscription_tier = self.custom_config.get('subscriptionTier', 'ultra')
            allow_automatic_payment = self.custom_config.get('allowAutomaticPayment', True)
            allow_trial = self.custom_config.get('allowTrial', True)
            
            print(f"{Fore.CYAN}{EMOJI['INFO']} 订阅配置: tier={subscription_tier}, autoPayment={allow_automatic_payment}, trial={allow_trial}{Style.RESET_ALL}")
            
            # 调用Rust API获取绑卡链接
            try:
                print(f"{Fore.CYAN}{EMOJI['INFO']} 调用Rust API获取绑卡链接...{Style.RESET_ALL}")
                bind_card_url = self._call_rust_api_for_bind_card_url(workos_token, subscription_tier, allow_automatic_payment, allow_trial)
                
                if bind_card_url:
                    print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 成功获取绑卡链接: {bind_card_url[:50]}...{Style.RESET_ALL}")
                    
                    # 跳转到绑卡页面
                    browser_tab.get(bind_card_url)
                    time.sleep(get_random_wait_time(self.config, 'payment_page_wait'))
                    
                    # 填写银行卡信息
                    self._fill_payment_form(browser_tab)
                    return True
                else:
                    print(f"{Fore.YELLOW}{EMOJI['WARNING']} API获取绑卡链接失败，回退到按钮点击方式{Style.RESET_ALL}")
                    # 如果API调用失败，回退到原有的按钮点击逻辑
                    return self._setup_payment_method_via_button_click(browser_tab)
                    
            except Exception as api_error:
                print(f"{Fore.YELLOW}{EMOJI['WARNING']} 调用API获取绑卡链接异常: {str(api_error)}，回退到按钮点击方式{Style.RESET_ALL}")
                # 如果API调用异常，回退到原有的按钮点击逻辑
                return self._setup_payment_method_via_button_click(browser_tab)
                
        except Exception as e:
            print(f"{Fore.YELLOW}{EMOJI['WARNING']} API绑卡流程出错: {str(e)}，回退到按钮点击方式{Style.RESET_ALL}")
            # 如果整个API流程出错，回退到原有的按钮点击逻辑
            return self._setup_payment_method_via_button_click(browser_tab)

    def _setup_payment_method_via_button_click(self, browser_tab):
        """通过按钮点击方式获取绑卡链接（原有逻辑）"""
        try:
            # 检查是否出现手机号验证页面
            print(f"{Fore.CYAN}{EMOJI['INFO']} 检查 dashboard 页面是否需要手机号验证...{Style.RESET_ALL}")
            if not wait_for_phone_verification(browser_tab, self.translator):
                print(f"{Fore.RED}{EMOJI['ERROR']} 手机号验证检查失败{Style.RESET_ALL}")
                return False

            # 查找并点击试用按钮（支持不同天数：7天、14天等）
            print(f"{Fore.CYAN}{EMOJI['INFO']} 查找试用按钮...{Style.RESET_ALL}")

            # 等待页面加载
            time.sleep(get_random_wait_time(self.config, 'page_load_wait'))

            # 查找包含 "Start X-day trial" 文本的按钮（使用正则匹配任意天数）
            trial_button = None
            trial_days = None
            try:
                # 方法1: 使用XPath正则匹配任意天数的试用按钮
                trial_button = browser_tab.ele("xpath://button[.//span[contains(text(), 'day trial')]]", timeout=10)
                if trial_button:
                    # 提取天数信息
                    button_text = trial_button.text
                    import re
                    match = re.search(r'(\d+)-day trial', button_text)
                    trial_days = match.group(1) if match else "未知"
            except:
                try:
                    # 方法2: 查找所有按钮，然后使用正则检查内容
                    buttons = browser_tab.eles("tag:button")
                    import re
                    for button in buttons:
                        if button.text:
                            # 匹配 "Start X-day trial" 模式，X可以是任意数字
                            match = re.search(r'(\d+)-day trial', button.text)
                            if match:
                                trial_button = button
                                trial_days = match.group(1)
                                break
                except:
                    pass

            if trial_button:
                print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 找到试用按钮: {trial_days}天试用{Style.RESET_ALL}")
                
                # 点击试用按钮
                trial_button.click()
                print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 已点击试用按钮{Style.RESET_ALL}")
                
                # 等待页面跳转到支付页面
                time.sleep(get_random_wait_time(self.config, 'payment_page_wait'))
                
                # 检查是否成功跳转到支付页面
                print(f"{Fore.CYAN}{EMOJI['INFO']} 当前页面URL: {browser_tab.url}{Style.RESET_ALL}")
                
                # 检查页面是否包含支付相关内容
                page_content = browser_tab.html.lower()
                if "stripe" in page_content or "payment" in page_content or "card" in page_content:
                    print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 成功跳转到支付页面{Style.RESET_ALL}")
                else:
                    print(f"{Fore.YELLOW}{EMOJI['WARNING']} 可能未正确跳转到支付页面，但继续尝试...{Style.RESET_ALL}")
                
                # 等待支付表单加载
                time.sleep(get_random_wait_time(self.config, 'form_load_wait'))
                
                # 在填写银行卡信息之前，先处理按钮点击逻辑
                # 添加重试机制：如果找不到按钮，刷新页面重试
                max_refresh_attempts = 5  # 最多刷新5次
                refresh_attempt = 0
                all_buttons = []
                
                while refresh_attempt < max_refresh_attempts:
                    try:
                        # 先查看页面上有哪些按钮
                        print(f"{Fore.CYAN}{EMOJI['INFO']} 分析页面上的所有按钮...{Style.RESET_ALL}")
                        all_buttons = browser_tab.eles("tag:button")
                        print(f"{Fore.CYAN}{EMOJI['INFO']} 找到 {len(all_buttons)} 个按钮{Style.RESET_ALL}")
                        
                        # 如果找到了按钮，跳出循环
                        if len(all_buttons) > 0:
                            print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 页面渲染正常，找到按钮{Style.RESET_ALL}")
                            break
                        else:
                            # 没找到按钮，可能页面渲染不完整
                            refresh_attempt += 1
                            print(f"{Fore.YELLOW}{EMOJI['WARNING']} 页面未找到任何按钮，可能渲染不完整（第{refresh_attempt}次尝试）{Style.RESET_ALL}")
                            
                            if refresh_attempt < max_refresh_attempts:
                                print(f"{Fore.CYAN}{EMOJI['INFO']} 刷新页面重试...{Style.RESET_ALL}")
                                browser_tab.refresh()
                                # 等待页面加载完毕
                                time.sleep(get_random_wait_time(self.config, 'page_load_wait'))
                                print(f"{Fore.CYAN}{EMOJI['INFO']} 页面刷新完成，重新查找按钮...{Style.RESET_ALL}")
                            else:
                                print(f"{Fore.RED}{EMOJI['ERROR']} 已达到最大刷新次数，继续执行后续步骤...{Style.RESET_ALL}")
                    
                    except Exception as find_err:
                        print(f"{Fore.YELLOW}{EMOJI['WARNING']} 查找按钮时出错: {str(find_err)}{Style.RESET_ALL}")
                        refresh_attempt += 1
                        if refresh_attempt < max_refresh_attempts:
                            print(f"{Fore.CYAN}{EMOJI['INFO']} 刷新页面重试...{Style.RESET_ALL}")
                            browser_tab.refresh()
                            # 等待页面加载完毕
                            time.sleep(get_random_wait_time(self.config, 'page_load_wait'))
                        else:
                            break
                
                # 如果最终找到了按钮，进行后续操作
                if len(all_buttons) > 0:
                    try:
                        for i, button in enumerate(all_buttons[:10]):  # 只显示前10个按钮
                            try:
                                button_text = button.text or ""
                                aria_label = button.attr("aria-label") or ""
                                data_testid = button.attr("data-testid") or ""
                                class_name = button.attr("class") or ""
                                print(f"{Fore.CYAN}  按钮 {i+1}: text='{button_text}', aria-label='{aria_label}', data-testid='{data_testid}', class='{class_name[:50]}...'{Style.RESET_ALL}")
                            except Exception as btn_err:
                                print(f"{Fore.YELLOW}  按钮 {i+1}: 获取属性失败 - {str(btn_err)}{Style.RESET_ALL}")

                        try:
                            # 从配置中获取按钮索引，默认为1（第二个按钮）
                            btn_index = self.custom_config.get('btnIndex', 1)
                            print(f"{Fore.CYAN}{EMOJI['INFO']} 使用按钮索引: {btn_index}{Style.RESET_ALL}")
                            
                            # 查找包含特定属性的按钮
                            if btn_index < len(all_buttons):
                                pay_with_card_button = all_buttons[btn_index]
                            else:
                                print(f"{Fore.YELLOW}{EMOJI['WARNING']} 按钮索引 {btn_index} 超出范围，使用默认索引 1{Style.RESET_ALL}")
                                pay_with_card_button = all_buttons[1]
                            if pay_with_card_button:
                                pay_with_card_button.click()
                            print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 执行成功{Style.RESET_ALL}")
                            time.sleep(1)
                        except Exception as e:
                            print(f"{Fore.YELLOW}{EMOJI['WARNING']} 方法1失败: {str(e)}{Style.RESET_ALL}")
                    
                    except Exception as main_err:
                        print(f"{Fore.RED}{EMOJI['ERROR']} 查找按钮过程中发生错误: {str(main_err)}{Style.RESET_ALL}")
                        print(f"{Fore.YELLOW}{EMOJI['WARNING']} 错误类型: {type(main_err).__name__}{Style.RESET_ALL}")
                        print(f"{Fore.CYAN}{EMOJI['INFO']} 忽略错误，直接尝试查找输入框...{Style.RESET_ALL}")
                        # 等待一下让页面稳定
                        time.sleep(2)
                else:
                    print(f"{Fore.YELLOW}{EMOJI['WARNING']} 未找到任何按钮，直接尝试查找输入框...{Style.RESET_ALL}")
                    time.sleep(2)

                # 现在尝试查找银行卡输入框
                print(f"{Fore.CYAN}{EMOJI['INFO']} 查找银行卡号输入框...{Style.RESET_ALL}")
                try:
                    card_number_input = browser_tab.ele("#cardNumber", timeout=15)
                    if card_number_input:
                        print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 找到银行卡号输入框{Style.RESET_ALL}")
                        # 尝试填写表单，但不管成功失败都继续
                        self._fill_payment_form(browser_tab)
                    else:
                        print(f"{Fore.YELLOW}{EMOJI['WARNING']} 银行卡信息页面未正确加载，未找到 #cardNumber 元素{Style.RESET_ALL}")
                        print(f"{Fore.CYAN}{EMOJI['INFO']} 继续尝试其他方式填写...{Style.RESET_ALL}")
                        
                        # 尝试查找其他可能的元素
                        print(f"{Fore.CYAN}{EMOJI['INFO']} 尝试查找其他支付相关元素...{Style.RESET_ALL}")
                        payment_elements = browser_tab.eles("input[type='text']")
                        print(f"{Fore.CYAN}{EMOJI['INFO']} 找到 {len(payment_elements)} 个文本输入框{Style.RESET_ALL}")

                        # 打印页面源码的一部分用于调试
                        page_source = browser_tab.html[:2000]  # 只取前2000个字符
                        print(f"{Fore.CYAN}{EMOJI['INFO']} 页面源码片段: {page_source}...{Style.RESET_ALL}")
                        
                        # 继续尝试填写表单
                        self._fill_payment_form(browser_tab)
                except Exception as e:
                    print(f"{Fore.YELLOW}{EMOJI['WARNING']} 查找银行卡输入框时出错: {str(e)}，继续执行...{Style.RESET_ALL}")
                    # 不管出错与否，都尝试填写表单
                    try:
                        self._fill_payment_form(browser_tab)
                    except Exception as form_error:
                        print(f"{Fore.YELLOW}{EMOJI['WARNING']} 填写表单出错: {str(form_error)}，继续执行...{Style.RESET_ALL}")
                
                # 不管成功失败，都返回True继续后续流程
                print(f"{Fore.CYAN}{EMOJI['INFO']} 银行卡绑定流程已执行，继续后续步骤...{Style.RESET_ALL}")
                return True
                
            else:
                print(f"{Fore.YELLOW}{EMOJI['WARNING']} 未找到试用按钮，但继续执行后续步骤...{Style.RESET_ALL}")
                # 即使没找到按钮，也尝试填写表单
                try:
                    self._fill_payment_form(browser_tab)
                except Exception as e:
                    print(f"{Fore.YELLOW}{EMOJI['WARNING']} 尝试填写表单出错: {str(e)}，继续执行...{Style.RESET_ALL}")
                return True

        except Exception as e:
            print(f"{Fore.YELLOW}{EMOJI['WARNING']} 设置支付方式过程出错: {str(e)}，但继续执行后续步骤...{Style.RESET_ALL}")
            # 即使出错也不返回False，继续执行
            try:
                self._fill_payment_form(browser_tab)
            except Exception as form_error:
                print(f"{Fore.YELLOW}{EMOJI['WARNING']} 尝试填写表单出错: {str(form_error)}，继续执行...{Style.RESET_ALL}")
            return True

    def _get_workos_session_token(self, browser_tab):
        """获取WorkOS session token"""
        try:
            # 尝试从cookie中获取WorkosCursorSessionToken
            cookies = browser_tab.cookies()
            for cookie in cookies:
                if cookie.get('name') == 'WorkosCursorSessionToken':
                    token = cookie.get('value')
                    print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 找到WorkOS session token: {token[:20]}...{Style.RESET_ALL}")
                    return token
            
            print(f"{Fore.RED}{EMOJI['ERROR']} 未找到WorkOS session token{Style.RESET_ALL}")
            return None
            
        except Exception as e:
            print(f"{Fore.RED}{EMOJI['ERROR']} 获取WorkOS session token失败: {str(e)}{Style.RESET_ALL}")
            return None

    def _call_rust_api_for_bind_card_url(self, workos_token, subscription_tier, allow_automatic_payment, allow_trial):
        """直接调用Cursor API获取绑卡链接（与Rust实现保持一致）"""
        try:
            import requests
            import json
            
            print(f"{Fore.CYAN}{EMOJI['INFO']} 直接调用Cursor API获取绑卡链接...{Style.RESET_ALL}")
            
            # 构建请求头（与Rust版本保持一致）
            headers = {
                'Cookie': f'WorkosCursorSessionToken={workos_token}',
                'Content-Type': 'application/json'
            }
            
            # 构建请求体
            body = {
                "tier": subscription_tier,
                "allowTrial": allow_trial,
                "allowAutomaticPayment": allow_automatic_payment
            }
            
            print(f"{Fore.CYAN}{EMOJI['INFO']} 请求参数: tier={subscription_tier}, allowTrial={allow_trial}, allowAutomaticPayment={allow_automatic_payment}{Style.RESET_ALL}")
            
            # 发送POST请求到Cursor API
            print(f"{Fore.CYAN}{EMOJI['INFO']} 发送请求到 https://cursor.com/api/checkout{Style.RESET_ALL}")
            
            response = requests.post(
                'https://cursor.com/api/checkout',
                headers=headers,
                json=body,
                timeout=30
            )
            
            print(f"{Fore.CYAN}{EMOJI['INFO']} 响应状态码: {response.status_code}{Style.RESET_ALL}")
            
            if response.status_code == 200:
                # 获取响应文本（直接就是URL，可能带引号）
                url = response.text.strip().strip('"')
                
                print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 成功获取绑卡链接: {url[:50]}...{Style.RESET_ALL}")
                
                # 检查是否返回的是 dashboard 页面（说明已经绑卡）
                if "cursor.com/dashboard" in url:
                    error_msg = "该账户可能已经绑定过银行卡，无法再次绑卡。如需更换银行卡，请先取消订阅后再试。"
                    print(f"{Fore.RED}{EMOJI['ERROR']} {error_msg}{Style.RESET_ALL}")
                    return None
                
                # 检查是否是 Stripe checkout URL
                if "checkout.stripe.com" not in url:
                    print(f"{Fore.RED}{EMOJI['ERROR']} 返回的不是有效的绑卡链接: {url}{Style.RESET_ALL}")
                    return None
                
                return url
            else:
                error_text = response.text
                print(f"{Fore.RED}{EMOJI['ERROR']} API请求失败: {response.status_code} - {error_text}{Style.RESET_ALL}")
                return None
                
        except requests.exceptions.RequestException as e:
            print(f"{Fore.RED}{EMOJI['ERROR']} 网络请求失败: {str(e)}{Style.RESET_ALL}")
            return None
        except Exception as e:
            print(f"{Fore.RED}{EMOJI['ERROR']} 调用API失败: {str(e)}{Style.RESET_ALL}")
            return None

    def _fill_payment_form(self, browser_tab):
        """填写银行卡信息表单"""
        try:
            print(f"{Fore.CYAN}{EMOJI['INFO']} 开始填写银行卡信息...{Style.RESET_ALL}")
            print(f"{Fore.CYAN}{EMOJI['INFO']} 当前页面URL: {browser_tab.url}{Style.RESET_ALL}")

            # 从配置文件读取银行卡信息
            card_info = self._load_bank_card_config()
            if not card_info:
                print(f"{Fore.RED}{EMOJI['ERROR']} 无法加载银行卡配置，使用默认配置{Style.RESET_ALL}")
                # 使用默认配置作为后备
                card_info = {
                    'cardNumber': '545046940484xxxx',
                    'cardExpiry': '08/30',
                    'cardCvc': '603',
                    'billingName': 'xxx xx',
                    'billingCountry': 'China',
                    'billingPostalCode': '494364',
                    'billingAdministrativeArea': '福建省 — Fujian Sheng',
                    'billingLocality': '福州市',
                    'billingDependentLocality': '闽侯县',
                    'billingAddressLine1': '银泰路201号'
                }
            
            print(f"{Fore.CYAN}{EMOJI['INFO']} 使用银行卡配置: {card_info['cardNumber'][:4]}****{card_info['cardNumber'][-4:]}{Style.RESET_ALL}")
            print(f"{Fore.CYAN}{EMOJI['INFO']} 持卡人: {card_info['billingName']}{Style.RESET_ALL}")

            # 在填写卡号之前，先处理按钮点击逻辑
            # 添加重试机制：如果找不到按钮，刷新页面重试
            max_refresh_attempts = 5  # 最多刷新5次
            refresh_attempt = 0
            all_buttons = []
            
            while refresh_attempt < max_refresh_attempts:
                try:
                    # 先查看页面上有哪些按钮
                    print(f"{Fore.CYAN}{EMOJI['INFO']} 分析页面上的所有按钮...{Style.RESET_ALL}")
                    all_buttons = browser_tab.eles("tag:button")
                    print(f"{Fore.CYAN}{EMOJI['INFO']} 找到 {len(all_buttons)} 个按钮{Style.RESET_ALL}")
                    
                    # 如果找到了按钮，跳出循环
                    if len(all_buttons) > 0:
                        print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 页面渲染正常，找到按钮{Style.RESET_ALL}")
                        break
                    else:
                        # 没找到按钮，可能页面渲染不完整
                        refresh_attempt += 1
                        print(f"{Fore.YELLOW}{EMOJI['WARNING']} 页面未找到任何按钮，可能渲染不完整（第{refresh_attempt}次尝试）{Style.RESET_ALL}")
                        
                        if refresh_attempt < max_refresh_attempts:
                            print(f"{Fore.CYAN}{EMOJI['INFO']} 刷新页面重试...{Style.RESET_ALL}")
                            browser_tab.refresh()
                            # 等待页面加载完毕
                            time.sleep(get_random_wait_time(self.config, 'page_load_wait'))
                            print(f"{Fore.CYAN}{EMOJI['INFO']} 页面刷新完成，重新查找按钮...{Style.RESET_ALL}")
                        else:
                            print(f"{Fore.RED}{EMOJI['ERROR']} 已达到最大刷新次数，继续执行后续步骤...{Style.RESET_ALL}")
                
                except Exception as find_err:
                    print(f"{Fore.YELLOW}{EMOJI['WARNING']} 查找按钮时出错: {str(find_err)}{Style.RESET_ALL}")
                    refresh_attempt += 1
                    if refresh_attempt < max_refresh_attempts:
                        print(f"{Fore.CYAN}{EMOJI['INFO']} 刷新页面重试...{Style.RESET_ALL}")
                        browser_tab.refresh()
                        # 等待页面加载完毕
                        time.sleep(get_random_wait_time(self.config, 'page_load_wait'))
                    else:
                        break
            
            # 如果最终找到了按钮，进行后续操作
            if len(all_buttons) > 0:
                try:
                    for i, button in enumerate(all_buttons[:10]):  # 只显示前10个按钮
                        try:
                            button_text = button.text or ""
                            aria_label = button.attr("aria-label") or ""
                            data_testid = button.attr("data-testid") or ""
                            class_name = button.attr("class") or ""
                            print(f"{Fore.CYAN}  按钮 {i+1}: text='{button_text}', aria-label='{aria_label}', data-testid='{data_testid}', class='{class_name[:50]}...'{Style.RESET_ALL}")
                        except Exception as btn_err:
                            print(f"{Fore.YELLOW}  按钮 {i+1}: 获取属性失败 - {str(btn_err)}{Style.RESET_ALL}")

                    try:
                        # 从配置中获取按钮索引，默认为0（银行卡）
                        btn_index = self.custom_config.get('btnIndex', 0)
                        print(f"{Fore.CYAN}{EMOJI['INFO']} 使用按钮索引: {btn_index}{Style.RESET_ALL}")
                        
                        pay_with_card_button = None
                        
                        # 根据btnIndex智能选择按钮
                        if btn_index == 0:
                            # 银行卡支付
                            print(f"{Fore.CYAN}{EMOJI['INFO']} 尝试查找银行卡按钮 (data-testid='card-accordion-item-button'){Style.RESET_ALL}")
                            for button in all_buttons:
                                if button.attr("data-testid") == "card-accordion-item-button":
                                    pay_with_card_button = button
                                    print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 找到银行卡按钮{Style.RESET_ALL}")
                                    break
                        elif btn_index == 1:
                            # 美国银行账户
                            print(f"{Fore.CYAN}{EMOJI['INFO']} 尝试查找美国银行账户按钮 (data-testid='us_bank_account-accordion-item-button'){Style.RESET_ALL}")
                            for button in all_buttons:
                                if button.attr("data-testid") == "us_bank_account-accordion-item-button":
                                    pay_with_card_button = button
                                    print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 找到美国银行账户按钮{Style.RESET_ALL}")
                                    break
                        
                        # 如果没有找到特定的按钮，使用索引作为fallback
                        if not pay_with_card_button:
                            print(f"{Fore.YELLOW}{EMOJI['WARNING']} 未找到特定按钮，使用索引 {btn_index} 作为fallback{Style.RESET_ALL}")
                            if btn_index < len(all_buttons):
                                pay_with_card_button = all_buttons[btn_index]
                            else:
                                print(f"{Fore.YELLOW}{EMOJI['WARNING']} 按钮索引 {btn_index} 超出范围，使用默认索引 0{Style.RESET_ALL}")
                                pay_with_card_button = all_buttons[0] if all_buttons else None
                        
                        if pay_with_card_button:
                            pay_with_card_button.click()
                            print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 按钮点击成功{Style.RESET_ALL}")
                        else:
                            print(f"{Fore.RED}{EMOJI['ERROR']} 未找到可用的按钮{Style.RESET_ALL}")
                        time.sleep(1)
                    except Exception as e:
                        print(f"{Fore.YELLOW}{EMOJI['WARNING']} 方法1失败: {str(e)}{Style.RESET_ALL}")
                
                except Exception as main_err:
                    print(f"{Fore.RED}{EMOJI['ERROR']} 查找按钮过程中发生错误: {str(main_err)}{Style.RESET_ALL}")
                    print(f"{Fore.YELLOW}{EMOJI['WARNING']} 错误类型: {type(main_err).__name__}{Style.RESET_ALL}")
                    print(f"{Fore.CYAN}{EMOJI['INFO']} 忽略错误，直接尝试查找输入框...{Style.RESET_ALL}")
                    # 等待一下让页面稳定
                    time.sleep(2)
            else:
                print(f"{Fore.YELLOW}{EMOJI['WARNING']} 未找到任何按钮，直接尝试查找输入框...{Style.RESET_ALL}")
                time.sleep(2)

            # 填写卡号
            print(f"{Fore.CYAN}{EMOJI['INFO']} 查找卡号输入框 #cardNumber...{Style.RESET_ALL}")
            try:
                card_number_input = browser_tab.ele("#cardNumber", timeout=5)
                if card_number_input:
                    print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 找到卡号输入框，开始填写...{Style.RESET_ALL}")
                    card_number_input.clear()
                    card_number_input.input(card_info['cardNumber'])
                    time.sleep(get_random_wait_time(self.config, 'input_wait'))
                else:
                    print(f"{Fore.YELLOW}{EMOJI['WARNING']} 未找到卡号输入框 #cardNumber，跳过...{Style.RESET_ALL}")
            except Exception as e:
                print(f"{Fore.YELLOW}{EMOJI['WARNING']} 填写卡号失败: {str(e)}，继续下一步...{Style.RESET_ALL}")

            # 填写有效期
            print(f"{Fore.CYAN}{EMOJI['INFO']} 查找有效期输入框 #cardExpiry...{Style.RESET_ALL}")
            try:
                card_expiry_input = browser_tab.ele("#cardExpiry", timeout=5)
                if card_expiry_input:
                    print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 找到有效期输入框，开始填写...{Style.RESET_ALL}")
                    card_expiry_input.clear()
                    card_expiry_input.input(card_info['cardExpiry'])
                    time.sleep(get_random_wait_time(self.config, 'input_wait'))
                else:
                    print(f"{Fore.YELLOW}{EMOJI['WARNING']} 未找到有效期输入框 #cardExpiry，跳过...{Style.RESET_ALL}")
            except Exception as e:
                print(f"{Fore.YELLOW}{EMOJI['WARNING']} 填写有效期失败: {str(e)}，继续下一步...{Style.RESET_ALL}")

            # 填写CVC
            print(f"{Fore.CYAN}{EMOJI['INFO']} 查找CVC输入框 #cardCvc...{Style.RESET_ALL}")
            try:
                card_cvc_input = browser_tab.ele("#cardCvc", timeout=5)
                if card_cvc_input:
                    print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 找到CVC输入框，开始填写...{Style.RESET_ALL}")
                    card_cvc_input.clear()
                    card_cvc_input.input(card_info['cardCvc'])
                    time.sleep(get_random_wait_time(self.config, 'input_wait'))
                else:
                    print(f"{Fore.YELLOW}{EMOJI['WARNING']} 未找到CVC输入框 #cardCvc，跳过...{Style.RESET_ALL}")
            except Exception as e:
                print(f"{Fore.YELLOW}{EMOJI['WARNING']} 填写CVC失败: {str(e)}，继续下一步...{Style.RESET_ALL}")

            # 填写持卡人姓名
            print(f"{Fore.CYAN}{EMOJI['INFO']} 查找持卡人姓名输入框 #billingName...{Style.RESET_ALL}")
            try:
                billing_name_input = browser_tab.ele("#billingName", timeout=5)
                if billing_name_input:
                    print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 找到持卡人姓名输入框，开始填写...{Style.RESET_ALL}")
                    billing_name_input.clear()
                    billing_name_input.input(card_info['billingName'])
                    time.sleep(get_random_wait_time(self.config, 'input_wait'))
                else:
                    print(f"{Fore.YELLOW}{EMOJI['WARNING']} 未找到持卡人姓名输入框，跳过...{Style.RESET_ALL}")
            except Exception as e:
                print(f"{Fore.YELLOW}{EMOJI['WARNING']} 填写持卡人姓名失败: {str(e)}，继续下一步...{Style.RESET_ALL}")

                      # 根据国家决定填写哪些字段
            is_china = card_info['billingCountry'].lower() == 'china'
            print(f"{Fore.CYAN}{EMOJI['INFO']} 检测到国家: {card_info['billingCountry']}, 中国模式: {is_china}{Style.RESET_ALL}")
            
            if is_china:
                # 中国需要填写详细信息
                # 填写邮政编码
                print(f"{Fore.CYAN}{EMOJI['INFO']} 查找邮政编码输入框 #billingPostalCode...{Style.RESET_ALL}")
                try:
                    postal_code_input = browser_tab.ele("#billingPostalCode", timeout=5)
                    if postal_code_input:
                        print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 找到邮政编码输入框，开始填写...{Style.RESET_ALL}")
                        postal_code_input.clear()
                        postal_code_input.input(card_info['billingPostalCode'])
                        time.sleep(get_random_wait_time(self.config, 'input_wait'))
                        print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 邮政编码填写完成{Style.RESET_ALL}")
                    else:
                        print(f"{Fore.YELLOW}{EMOJI['WARNING']} 未找到邮政编码输入框，跳过...{Style.RESET_ALL}")
                except Exception as e:
                    print(f"{Fore.YELLOW}{EMOJI['WARNING']} 填写邮政编码失败: {str(e)}，继续下一步...{Style.RESET_ALL}")

                # 选择省份
                print(f"{Fore.CYAN}{EMOJI['INFO']} 查找省份选择框 #billingAdministrativeArea...{Style.RESET_ALL}")
                try:
                    province_select = browser_tab.ele("#billingAdministrativeArea", timeout=5)
                    if province_select:
                        print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 找到省份选择框，开始选择...{Style.RESET_ALL}")
                        province_select.select(card_info['billingAdministrativeArea'])
                        time.sleep(get_random_wait_time(self.config, 'input_wait'))
                        print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 省份选择完成{Style.RESET_ALL}")
                    else:
                        print(f"{Fore.YELLOW}{EMOJI['WARNING']} 未找到省份选择框，跳过...{Style.RESET_ALL}")
                except Exception as e:
                    print(f"{Fore.YELLOW}{EMOJI['WARNING']} 省份选择失败: {str(e)}，继续下一步...{Style.RESET_ALL}")

                # 填写城市
                print(f"{Fore.CYAN}{EMOJI['INFO']} 查找城市输入框 #billingLocality...{Style.RESET_ALL}")
                try:
                    city_input = browser_tab.ele("#billingLocality", timeout=5)
                    if city_input:
                        print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 找到城市输入框，开始填写...{Style.RESET_ALL}")
                        city_input.clear()
                        city_input.input(card_info['billingLocality'])
                        time.sleep(get_random_wait_time(self.config, 'input_wait'))
                        print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 城市填写完成{Style.RESET_ALL}")
                    else:
                        print(f"{Fore.YELLOW}{EMOJI['WARNING']} 未找到城市输入框，跳过...{Style.RESET_ALL}")
                except Exception as e:
                    print(f"{Fore.YELLOW}{EMOJI['WARNING']} 填写城市失败: {str(e)}，继续下一步...{Style.RESET_ALL}")

                # 填写区县
                print(f"{Fore.CYAN}{EMOJI['INFO']} 查找区县输入框 #billingDependentLocality...{Style.RESET_ALL}")
                try:
                    district_input = browser_tab.ele("#billingDependentLocality", timeout=5)
                    if district_input:
                        print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 找到区县输入框，开始填写...{Style.RESET_ALL}")
                        district_input.clear()
                        district_input.input(card_info['billingDependentLocality'])
                        time.sleep(get_random_wait_time(self.config, 'input_wait'))
                        print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 区县填写完成{Style.RESET_ALL}")
                    else:
                        print(f"{Fore.YELLOW}{EMOJI['WARNING']} 未找到区县输入框，跳过...{Style.RESET_ALL}")
                except Exception as e:
                    print(f"{Fore.YELLOW}{EMOJI['WARNING']} 填写区县失败: {str(e)}，继续下一步...{Style.RESET_ALL}")

                # 填写地址
                print(f"{Fore.CYAN}{EMOJI['INFO']} 查找地址输入框 #billingAddressLine1...{Style.RESET_ALL}")
                try:
                    address_input = browser_tab.ele("#billingAddressLine1", timeout=5)
                    if address_input:
                        print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 找到地址输入框，开始填写...{Style.RESET_ALL}")
                        address_input.clear()
                        address_input.input(card_info['billingAddressLine1'])
                        time.sleep(get_random_wait_time(self.config, 'input_wait'))
                        print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 地址填写完成{Style.RESET_ALL}")
                    else:
                        print(f"{Fore.YELLOW}{EMOJI['WARNING']} 未找到地址输入框，跳过...{Style.RESET_ALL}")
                except Exception as e:
                    print(f"{Fore.YELLOW}{EMOJI['WARNING']} 填写地址失败: {str(e)}，继续下一步...{Style.RESET_ALL}")
            else:
                # 非中国只需要填写地址，填写完成后不自动提交
                print(f"{Fore.CYAN}{EMOJI['INFO']} 非中国地址，只填写地址字段...{Style.RESET_ALL}")
                print(f"{Fore.CYAN}{EMOJI['INFO']} 查找地址输入框 #billingAddressLine1...{Style.RESET_ALL}")
                try:
                    address_input = browser_tab.ele("#billingAddressLine1", timeout=5)
                    if address_input:
                        print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 找到地址输入框，开始填写...{Style.RESET_ALL}")
                        address_input.clear()
                        address_input.input(card_info['billingAddressLine1'])
                        time.sleep(3)  # 等待3秒
                        print(f"{Fore.CYAN}{EMOJI['INFO']} 触发Enter事件...{Style.RESET_ALL}")
                        address_input.input('\n')  # 触发Enter事件
                        print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 地址填写完成并触发Enter事件{Style.RESET_ALL}")
                        
                        # 非中国地址填写完成后，等待用户手动填写其他信息
                        print(f"{Fore.YELLOW}{EMOJI['INFO']} 非中国地址填写完成，请手动填写其他必要的地址信息{Style.RESET_ALL}")
                        print(f"{Fore.YELLOW}{EMOJI['INFO']} 填写完成后请手动提交表单，浏览器将保持打开状态{Style.RESET_ALL}")
                        
                        # 返回特殊状态，表示非中国地址填写完成，需要保持浏览器打开
                        return "non_china_completed"
                    else:
                        print(f"{Fore.YELLOW}{EMOJI['WARNING']} 未找到地址输入框，浏览器将保持打开状态以便手动操作{Style.RESET_ALL}")
                        return "non_china_completed"
                except Exception as e:
                    print(f"{Fore.YELLOW}{EMOJI['WARNING']} 填写地址失败: {str(e)}，浏览器将保持打开状态以便手动操作{Style.RESET_ALL}")
                    return "non_china_completed"

            print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 银行卡信息填写完成！{Style.RESET_ALL}")
            
            time.sleep(5)

            # 中国地址自动提交，但返回特殊状态保持浏览器打开
            try:
                submit_result = self._submit_payment_form(browser_tab)
                if submit_result:
                    print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 表单已提交{Style.RESET_ALL}")
                    print(f"{Fore.YELLOW}{EMOJI['INFO']} 浏览器将保持打开状态，请查看提交结果{Style.RESET_ALL}")
                else:
                    print(f"{Fore.YELLOW}{EMOJI['WARNING']} 表单提交失败，浏览器将保持打开状态以便手动操作{Style.RESET_ALL}")
            except Exception as submit_error:
                print(f"{Fore.YELLOW}{EMOJI['WARNING']} 表单提交出错: {str(submit_error)}，浏览器将保持打开状态以便手动操作{Style.RESET_ALL}")
            
            # 不管成功失败，都返回特殊状态保持浏览器打开
            return "non_china_completed"

        except Exception as e:
            print(f"{Fore.RED}{EMOJI['ERROR']} 填写银行卡信息失败: {str(e)}{Style.RESET_ALL}")
            print(f"{Fore.YELLOW}{EMOJI['WARNING']} 等待10秒后继续...{Style.RESET_ALL}")
            time.sleep(10)
            print(f"{Fore.CYAN}{EMOJI['INFO']} 尽管填写过程中有错误，但可能部分信息已经填写成功{Style.RESET_ALL}")
            return True  # 返回True让流程继续，而不是立即失败

    def _submit_payment_form(self, browser_tab):
        """提交银行卡信息表单"""
        print(f"{Fore.CYAN}{EMOJI['INFO']} 查找最终提交按钮...{Style.RESET_ALL}")
        all_buttons = browser_tab.eles("tag:button")
        print(f"{Fore.CYAN}{EMOJI['INFO']} 找到 {len(all_buttons)} 个按钮{Style.RESET_ALL}")

        for i, button in enumerate(all_buttons[:10]):  # 只显示前10个按钮
            try:
                button_text = button.text or ""
                aria_label = button.attr("aria-label") or ""
                data_testid = button.attr("data-testid") or ""
                class_name = button.attr("class") or ""
                print(f"{Fore.CYAN}  按钮 {i+1}: text='{button_text}', aria-label='{aria_label}', data-testid='{data_testid}', class='{class_name[:50]}...'{Style.RESET_ALL}")
            except Exception as btn_err:
                print(f"{Fore.YELLOW}  按钮 {i+1}: 获取属性失败 - {str(btn_err)}{Style.RESET_ALL}")

        # 查找最终的提交按钮（可能是 "Complete payment" 或类似的按钮）
        try:
            submit_button = None
            
            # 优先通过 data-testid 查找提交按钮
            print(f"{Fore.CYAN}{EMOJI['INFO']} 尝试查找提交按钮 (data-testid='hosted-payment-submit-button'){Style.RESET_ALL}")
            for button in all_buttons:
                if button.attr("data-testid") == "hosted-payment-submit-button":
                    submit_button = button
                    print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 找到提交按钮 (data-testid){Style.RESET_ALL}")
                    break
            
            # 如果没找到，通过 class 查找
            if not submit_button:
                print(f"{Fore.CYAN}{EMOJI['INFO']} 尝试查找提交按钮 (class='SubmitButton SubmitButton--incomplete'){Style.RESET_ALL}")
                for button in all_buttons:
                    button_class = button.attr("class") or ""
                    if "SubmitButton SubmitButton--incomplete" in button_class:
                        submit_button = button
                        print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 找到提交按钮 (class){Style.RESET_ALL}")
                        break
            
            # 如果还是没找到，使用索引作为 fallback
            if not submit_button:
                print(f"{Fore.YELLOW}{EMOJI['WARNING']} 未找到特定提交按钮，使用索引 4 作为fallback{Style.RESET_ALL}")
                if len(all_buttons) > 4:
                    submit_button = all_buttons[4]
                elif len(all_buttons) > 0:
                    submit_button = all_buttons[-1]  # 使用最后一个按钮
                    print(f"{Fore.YELLOW}{EMOJI['WARNING']} 按钮数量不足，使用最后一个按钮{Style.RESET_ALL}")
            
            if submit_button:
                print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 找到提交按钮，点击...{Style.RESET_ALL}")
                submit_button.click()
                time.sleep(20)
                return True
            else:
                print(f"{Fore.RED}{EMOJI['ERROR']} 未找到任何可用的提交按钮{Style.RESET_ALL}")
                return False


        except Exception as e:
            print(f"{Fore.RED}{EMOJI['ERROR']} 提交银行卡信息失败: {str(e)}{Style.RESET_ALL}")
            return False

    def _load_bank_card_config(self):
        """从配置文件加载银行卡信息，从custom_config中获取cardIndex"""
        try:
            import json
            import os
            
            # 使用传递进来的应用目录，如果没有则回退到当前工作目录
            if self.app_dir:
                config_dir = self.app_dir
                print(f"{Fore.CYAN}{EMOJI['INFO']} 使用应用目录: {config_dir}{Style.RESET_ALL}")
            else:
                config_dir = os.getcwd()
                print(f"{Fore.YELLOW}{EMOJI['WARNING']} 应用目录未提供，使用当前工作目录: {config_dir}{Style.RESET_ALL}")
            
            config_path = os.path.join(config_dir, 'bank_card_config.json')
            
            print(f"{Fore.CYAN}{EMOJI['INFO']} 尝试加载银行卡配置文件: {config_path}{Style.RESET_ALL}")
            
            if not os.path.exists(config_path):
                print(f"{Fore.YELLOW}{EMOJI['WARNING']} 银行卡配置文件不存在: {config_path}{Style.RESET_ALL}")
                return None
            
            with open(config_path, 'r', encoding='utf-8') as f:
                config_data = json.load(f)
            
            # 检查是否有cards数组
            if 'cards' not in config_data or not isinstance(config_data['cards'], list):
                print(f"{Fore.RED}{EMOJI['ERROR']} 配置文件格式错误：缺少cards数组{Style.RESET_ALL}")
                return None
            
            cards = config_data['cards']
            if len(cards) == 0:
                print(f"{Fore.RED}{EMOJI['ERROR']} 配置文件中没有银行卡信息{Style.RESET_ALL}")
                return None
            
            # 从custom_config中获取cardIndex，默认为0
            card_index = self.custom_config.get('cardIndex', 0)
            print(f"{Fore.CYAN}{EMOJI['INFO']} 从配置中获取卡片索引: {card_index}{Style.RESET_ALL}")
            
            # 处理索引越界，默认使用第一张卡
            if card_index >= len(cards) or card_index < 0:
                print(f"{Fore.YELLOW}{EMOJI['WARNING']} 卡片索引 {card_index} 超出范围，使用第一张卡（索引0）{Style.RESET_ALL}")
                card_index = 0
            
            selected_card = cards[card_index]
            print(f"{Fore.CYAN}{EMOJI['INFO']} 选择使用卡片索引 {card_index}，卡号后四位: ****{selected_card.get('cardNumber', '')[-4:]}{Style.RESET_ALL}")
            
            # 验证必需的字段
            required_fields = [
                'cardNumber', 'cardExpiry', 'cardCvc', 'billingName', 
                'billingCountry', 'billingPostalCode', 'billingAdministrativeArea',
                'billingLocality', 'billingDependentLocality', 'billingAddressLine1'
            ]
            
            for field in required_fields:
                if field not in selected_card or not selected_card[field]:
                    print(f"{Fore.YELLOW}{EMOJI['WARNING']} 选中的卡片缺少必需字段: {field}{Style.RESET_ALL}")
                    return None
            
            print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 成功加载银行卡配置{Style.RESET_ALL}")
            return selected_card
            
        except json.JSONDecodeError as e:
            print(f"{Fore.RED}{EMOJI['ERROR']} 银行卡配置文件JSON格式错误: {str(e)}{Style.RESET_ALL}")
            return None
        except Exception as e:
            print(f"{Fore.RED}{EMOJI['ERROR']} 加载银行卡配置失败: {str(e)}{Style.RESET_ALL}")
            return None

    def _wait_for_user_completion(self, browser_tab):
        """等待用户手动完成地址填写和表单提交"""
        try:
            import tempfile
            
            print(f"\n{Fore.YELLOW}{'='*60}{Style.RESET_ALL}")
            print(f"{Fore.YELLOW}{EMOJI['INFO']} 等待用户手动操作...{Style.RESET_ALL}")
            print(f"{Fore.YELLOW}{EMOJI['INFO']} 请在浏览器中完成以下操作：{Style.RESET_ALL}")
            print(f"{Fore.YELLOW}  1. 填写必要的地址信息（邮编、州/省等）{Style.RESET_ALL}")
            print(f"{Fore.YELLOW}  2. 点击提交按钮完成银行卡绑定{Style.RESET_ALL}")
            print(f"{Fore.YELLOW}  3. 完成后可以关闭浏览器{Style.RESET_ALL}")
            print(f"{Fore.YELLOW}{'='*60}{Style.RESET_ALL}")
            
            # 保持进程运行，直到用户手动关闭
            print(f"{Fore.CYAN}{EMOJI['INFO']} 程序将保持运行状态...{Style.RESET_ALL}")
            print(f"{Fore.CYAN}{EMOJI['INFO']} 如需退出，请按 Ctrl+C 或关闭此窗口{Style.RESET_ALL}")
            
            # 获取停止信号文件路径（从环境变量获取，必须通过环境变量传递）
            temp_dir = tempfile.gettempdir()
            # 必须使用环境变量中指定的停止信号文件路径（包含task_id，用于并行注册隔离）
            stop_file = os.environ.get('CURSOR_REGISTRATION_STOP_FILE')
            
            if not stop_file:
                # 如果没有环境变量，说明配置错误，使用错误提示
                print(f"{Fore.RED}{EMOJI['ERROR']} 未找到停止信号文件环境变量 CURSOR_REGISTRATION_STOP_FILE{Style.RESET_ALL}")
                print(f"{Fore.YELLOW}{EMOJI['WARNING']} 无法正确检测停止信号，将使用默认机制{Style.RESET_ALL}")
                # 为了兼容，使用一个通用的文件名（但这种情况不应该发生）
                stop_file = os.path.join(temp_dir, "cursor_registration_stop.txt")
            
            print(f"{Fore.CYAN}{EMOJI['INFO']} 停止信号文件: {stop_file}{Style.RESET_ALL}")
            
            # 清理可能存在的旧停止文件
            if os.path.exists(stop_file):
                try:
                    os.remove(stop_file)
                    print(f"{Fore.YELLOW}{EMOJI['INFO']} 清理旧停止文件: {stop_file}{Style.RESET_ALL}")
                except Exception as e:
                    print(f"{Fore.RED}{EMOJI['ERROR']} 清理停止文件失败 {stop_file}: {e}{Style.RESET_ALL}")
            
            # 无限循环，保持进程运行
            while True:
                try:
                    # 检查是否有停止请求
                    if os.path.exists(stop_file):
                        try:
                            # 读取文件内容，确认是停止信号
                            with open(stop_file, 'r') as f:
                                content = f.read().strip()
                            
                            if content == "stop":
                                print(f"{Fore.YELLOW}{EMOJI['INFO']} 收到停止信号，准备关闭浏览器并退出...{Style.RESET_ALL}")
                                # 删除停止文件
                                try:
                                    os.remove(stop_file)
                                    print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 已清理停止文件{Style.RESET_ALL}")
                                except:
                                    pass
                                
                                # 关闭浏览器
                                if browser_tab:
                                    try:
                                        print(f"{Fore.CYAN}{EMOJI['INFO']} 正在关闭浏览器...{Style.RESET_ALL}")
                                        browser_tab.quit()
                                        print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 浏览器已关闭{Style.RESET_ALL}")
                                    except Exception as e:
                                        print(f"{Fore.YELLOW}{EMOJI['WARNING']} 关闭浏览器时出错: {str(e)}{Style.RESET_ALL}")
                                
                                # 输出账户信息并退出
                                print(f"{Fore.CYAN}{EMOJI['INFO']} 准备输出账户信息并退出...{Style.RESET_ALL}")
                                self._output_completion_info()
                                print(f"{Fore.GREEN}{EMOJI['SUCCESS']} 进程即将退出，可以继续下一个任务{Style.RESET_ALL}")
                                break
                        except Exception as e:
                            print(f"{Fore.YELLOW}{EMOJI['WARNING']} 读取停止文件时出错: {str(e)}{Style.RESET_ALL}")
                    
                    # 检查浏览器是否还在运行
                    if browser_tab:
                        # 每2秒检查一次（同时检查停止文件和浏览器状态）
                        time.sleep(2)
                        try:
                            # 尝试获取当前URL，如果失败说明浏览器可能已关闭
                            current_url = browser_tab.url
                            # 只在需要时输出URL，减少日志输出
                        except:
                            print(f"{Fore.YELLOW}{EMOJI['INFO']} 浏览器已关闭，准备结束进程...{Style.RESET_ALL}")
                            # 浏览器关闭时，输出正常的注册完成信息
                            self._output_completion_info()
                            break
                    else:
                        time.sleep(2)
                        # 减少日志输出频率
                except KeyboardInterrupt:
                    print(f"\n{Fore.YELLOW}{EMOJI['INFO']} 用户手动终止程序{Style.RESET_ALL}")
                    # 用户手动终止时也输出完成信息
                    self._output_completion_info()
                    break
                except Exception as e:
                    print(f"{Fore.YELLOW}{EMOJI['WARNING']} 检查浏览器状态时出错: {str(e)}{Style.RESET_ALL}")
                    print(f"{Fore.YELLOW}{EMOJI['INFO']} 程序将结束...{Style.RESET_ALL}")
                    # 出错时也输出完成信息
                    self._output_completion_info()
                    break
                    
        except Exception as e:
            print(f"{Fore.RED}{EMOJI['ERROR']} 等待用户操作时出错: {str(e)}{Style.RESET_ALL}")
            # 出错时也输出完成信息
            self._output_completion_info()

    def _output_completion_info(self):
        """输出注册完成信息，格式与正常注册一致，供前端捕获token"""
        try:
            # 获取已保存的账户信息
            if hasattr(self, 'account_info') and self.account_info:
                # 输出和正常注册完成时一样的JSON格式
                print(json.dumps(self.account_info))
            else:
                # 如果没有保存的账户信息，尝试重新获取
                print(f"{Fore.CYAN}{EMOJI['INFO']} 尝试获取账户信息...{Style.RESET_ALL}")
                if hasattr(self, 'signup_tab') and self.signup_tab:
                    try:
                        # 重新获取账户信息
                        self._get_account_info()
                        if hasattr(self, 'account_info') and self.account_info:
                            print(json.dumps(self.account_info))
                        else:
                            # 如果还是没有，输出基本的成功信息
                            basic_info = {
                                "success": True,
                                "email": getattr(self, 'email_address', 'unknown'),
                                "first_name": getattr(self, 'first_name', 'unknown'),
                                "last_name": getattr(self, 'last_name', 'unknown'),
                                "message": "注册成功",
                                "status": "completed"
                            }
                            print(json.dumps(basic_info))
                    except Exception as e:
                        print(f"{Fore.YELLOW}{EMOJI['WARNING']} 重新获取账户信息失败: {str(e)}{Style.RESET_ALL}")
                        # 输出基本的成功信息
                        basic_info = {
                            "success": True,
                            "email": getattr(self, 'email_address', 'unknown'),
                            "first_name": getattr(self, 'first_name', 'unknown'),
                            "last_name": getattr(self, 'last_name', 'unknown'),
                            "message": "注册成功",
                            "status": "completed"
                        }
                        print(json.dumps(basic_info))
                else:
                    # 输出基本的成功信息
                    basic_info = {
                        "success": True,
                        "email": getattr(self, 'email_address', 'unknown'),
                        "first_name": getattr(self, 'first_name', 'unknown'),
                        "last_name": getattr(self, 'last_name', 'unknown'),
                        "message": "注册成功",
                        "status": "completed"
                    }
                    print(json.dumps(basic_info))
                    
        except Exception as e:
            print(f"{Fore.RED}{EMOJI['ERROR']} 输出完成信息时出错: {str(e)}{Style.RESET_ALL}")
            # 即使出错也输出基本信息
            try:
                basic_info = {
                    "success": True,
                    "email": getattr(self, 'email_address', 'unknown'),
                    "first_name": getattr(self, 'first_name', 'unknown'),
                    "last_name": getattr(self, 'last_name', 'unknown'),
                    "message": "注册成功",
                    "status": "completed"
                }
                print(json.dumps(basic_info))
            except:
                pass

    def start(self):
        """Start Registration Process"""
        try:
            if self.setup_email():
                if self.register_cursor():
                    print(f"\n{Fore.GREEN}{EMOJI['DONE']} {self.translator.get('register.cursor_registration_completed')}...{Style.RESET_ALL}")
                    return True
            return False
        finally:
            # Close email tab
            if hasattr(self, 'temp_email'):
                try:
                    self.temp_email.close()
                except:
                    pass

    def update_cursor_auth(self, email=None, access_token=None, refresh_token=None, auth_type="Auth_0"):
        """Convenient function to update Cursor authentication information"""
        auth_manager = CursorAuth(translator=self.translator)
        return auth_manager.update_auth(email, access_token, refresh_token, auth_type)

def main(translator=None, app_dir=None, enable_bank_card_binding=True):
    """Main function to be called from main.py"""
    print(f"\n{Fore.CYAN}{'='*50}{Style.RESET_ALL}")
    print(f"{Fore.CYAN}{EMOJI['START']} {translator.get('register.title') if translator else 'Cursor Registration'}{Style.RESET_ALL}")
    print(f"{Fore.CYAN}{'='*50}{Style.RESET_ALL}")

    registration = CursorRegistration(translator, app_dir=app_dir, enable_bank_card_binding=enable_bank_card_binding)
    registration.start()

    print(f"\n{Fore.CYAN}{'='*50}{Style.RESET_ALL}")
    input(f"{EMOJI['INFO']} {translator.get('register.press_enter') if translator else 'Press Enter to continue...'}...")

if __name__ == "__main__":
    import sys
    import hashlib
    
    # 🔒 生成进程标识，用于追踪是否有重复启动
    process_id = hashlib.md5(str(os.getpid()).encode()).hexdigest()[:8]
    print(f"{Fore.CYAN}🔍 [进程 {process_id}] Python脚本已启动{Style.RESET_ALL}")
    print(f"{Fore.CYAN}🔍 [进程 {process_id}] PID: {os.getpid()}{Style.RESET_ALL}")
    print(f"{Fore.CYAN}🔍 [进程 {process_id}] 命令行参数数量: {len(sys.argv)}{Style.RESET_ALL}")
    
    # 检查是否有足够的命令行参数
    # 预期参数顺序: email, first_name, last_name, incognito_flag, app_dir, enable_bank_card_binding
    app_dir = None
    email = None
    first_name = None
    last_name = None
    use_incognito = True
    enable_bank_card_binding = True
    
    if len(sys.argv) >= 7:
        # 从 Rust 调用，有完整参数（包括银行卡绑定参数），但没有卡片索引（向后兼容）
        email = sys.argv[1]
        first_name = sys.argv[2]
        last_name = sys.argv[3]
        incognito_flag = sys.argv[4]
        app_dir_base64 = sys.argv[5]
        bank_card_flag = sys.argv[6]
        use_incognito = incognito_flag.lower() == "true"
        enable_bank_card_binding = bank_card_flag.lower() == "true"
        
        # 调试银行卡参数解析
        print(f"{Fore.CYAN}{EMOJI['INFO']} [DEBUG] 银行卡参数解析:")
        print(f"  - bank_card_flag 原始值: '{bank_card_flag}'")
        print(f"  - bank_card_flag.lower(): '{bank_card_flag.lower()}'")
        print(f"  - bank_card_flag.lower() == 'true': {bank_card_flag.lower() == 'true'}")
        print(f"  - enable_bank_card_binding 最终值: {enable_bank_card_binding}")
        
        # 解码 Base64 编码的应用目录
        try:
            from base64 import standard_b64decode
            app_dir = standard_b64decode(app_dir_base64).decode('utf-8')
        except Exception as e:
            print(f"{Fore.YELLOW}{EMOJI['WARNING']} Base64解码失败，使用原始路径: {str(e)}{Style.RESET_ALL}")
            app_dir = app_dir_base64
        
        print(f"{Fore.CYAN}{EMOJI['INFO']} [进程 {process_id}] 从 Rust 调用，参数: email={email}, name={first_name} {last_name}, incognito={use_incognito}, bank_card={enable_bank_card_binding}, app_dir={app_dir}{Style.RESET_ALL}")
        print(f"{Fore.YELLOW}⚠️ [进程 {process_id}] 注册邮箱: {email} - 请确认这是您想要注册的邮箱{Style.RESET_ALL}")
        
        # 创建注册实例并执行
        try:
            print(f"{Fore.CYAN}🔧 [进程 {process_id}] 创建 CursorRegistration 实例...{Style.RESET_ALL}")
            registration = CursorRegistration(translator=None, use_incognito=use_incognito, app_dir=app_dir, enable_bank_card_binding=enable_bank_card_binding)
            registration.email_address = email
            registration.first_name = first_name
            registration.last_name = last_name
            print(f"{Fore.GREEN}✅ [进程 {process_id}] CursorRegistration 实例已创建，开始注册流程...{Style.RESET_ALL}")
            
            # 直接调用注册流程
            success = registration.register_cursor()
            if success:
                print(f"{Fore.GREEN}{EMOJI['DONE']} 注册流程完成{Style.RESET_ALL}")
            else:
                print(f"{Fore.RED}{EMOJI['ERROR']} 注册流程失败{Style.RESET_ALL}")
                
        except Exception as e:
            print(f"{Fore.RED}{EMOJI['ERROR']} 注册过程中发生错误: {str(e)}{Style.RESET_ALL}")
    
    elif len(sys.argv) >= 6:
        # 从 Rust 调用，旧版本参数（向后兼容）
        email = sys.argv[1]
        first_name = sys.argv[2]
        last_name = sys.argv[3]
        incognito_flag = sys.argv[4]
        app_dir_base64 = sys.argv[5]
        use_incognito = incognito_flag.lower() == "true"
        # 默认启用银行卡绑定（向后兼容）
        enable_bank_card_binding = True
        
        # 解码 Base64 编码的应用目录
        try:
            from base64 import standard_b64decode
            app_dir = standard_b64decode(app_dir_base64).decode('utf-8')
        except Exception as e:
            print(f"{Fore.YELLOW}{EMOJI['WARNING']} Base64解码失败，使用原始路径: {str(e)}{Style.RESET_ALL}")
            app_dir = app_dir_base64
        
        print(f"{Fore.CYAN}{EMOJI['INFO']} 从 Rust 调用（旧版本），参数: email={email}, name={first_name} {last_name}, incognito={use_incognito}, app_dir={app_dir}{Style.RESET_ALL}")
        
        # 创建注册实例并执行
        try:
            registration = CursorRegistration(translator=None, use_incognito=use_incognito, app_dir=app_dir, enable_bank_card_binding=enable_bank_card_binding)
            registration.email_address = email
            registration.first_name = first_name
            registration.last_name = last_name
            
            # 直接调用注册流程
            success = registration.register_cursor()
            if success:
                print(f"{Fore.GREEN}{EMOJI['DONE']} 注册流程完成{Style.RESET_ALL}")
            else:
                print(f"{Fore.RED}{EMOJI['ERROR']} 注册流程失败{Style.RESET_ALL}")
                
        except Exception as e:
            print(f"{Fore.RED}{EMOJI['ERROR']} 注册过程中发生错误: {str(e)}{Style.RESET_ALL}")
    
    elif len(sys.argv) > 1:
        # 只有应用目录参数（向后兼容）
        app_dir = sys.argv[1]
        print(f"{Fore.CYAN}{EMOJI['INFO']} 从命令行参数获取应用目录: {app_dir}{Style.RESET_ALL}")
        
        try:
            from main import translator as main_translator
            main(main_translator, app_dir)
        except ImportError:
            # 如果无法导入main模块，使用默认的None
            main(None, app_dir)
    else:
        # 没有参数，交互式模式
        try:
            from main import translator as main_translator
            main(main_translator, None)
        except ImportError:
            # 如果无法导入main模块，使用默认的None
            main(None, None)