from DrissionPage import ChromiumOptions, ChromiumPage
import time
import os
import signal
import random
import json
import subprocess
from colorama import Fore, Style, init
import configparser
from pathlib import Path
import sys
from urllib.parse import urlparse
from config import get_config
from utils import get_default_browser_path as utils_get_default_browser_path

for _stream_name in ("stdout", "stderr"):
    _stream = getattr(sys, _stream_name, None)
    if _stream and hasattr(_stream, "reconfigure"):
        try:
            _stream.reconfigure(errors="replace")
        except Exception:
            pass

init(strip=False, convert=False, wrap=False)

# Add global variable at the beginning of the file
_translator = None

# Add global variable to track our Chrome processes
_chrome_process_ids = []

def cleanup_chrome_processes(translator=None):
    """Clean only Chrome processes launched by this script"""
    global _chrome_process_ids
    
    if not _chrome_process_ids:
        print("\nNo Chrome processes to clean...")
        return
        
    print("\nCleaning Chrome processes launched by this script...")
    try:
        if os.name == 'nt':
            for pid in _chrome_process_ids:
                try:
                    os.system(f'taskkill /F /PID {pid} /T 2>nul')
                except:
                    pass
        else:
            for pid in _chrome_process_ids:
                try:
                    os.kill(pid, signal.SIGTERM)
                except:
                    pass
        _chrome_process_ids = []  # Reset the list after cleanup
    except Exception as e:
        if translator:
            print(f"{Fore.RED}❌ {translator.get('register.cleanup_error', error=str(e))}{Style.RESET_ALL}")
        else:
            print(f"清理进程时出错: {e}")

def signal_handler(signum, frame):
    """Handle Ctrl+C signal"""
    global _translator
    if _translator:
        print(f"{Fore.CYAN}{_translator.get('register.exit_signal')}{Style.RESET_ALL}")
    else:
        print("\n接收到退出信号，正在关闭...")
    cleanup_chrome_processes(_translator)
    os._exit(0)

def _navigate_front_window_via_macos_applescript(url):
    """
    On macOS incognito mode, CDP can bind to a hidden target.
    Navigate the front window tab directly via AppleScript.
    """
    safe_url = json.dumps(url)
    script = f"""
tell application "Google Chrome"
    activate
    if (count of windows) is 0 then error "Google Chrome has no open windows"
    set URL of active tab of front window to {safe_url}
end tell
"""
    try:
        result = subprocess.run(
            ["osascript", "-e", script],
            check=False,
            capture_output=True,
            text=True,
            timeout=8,
        )
    except Exception as e:
        print(f"{Fore.YELLOW}⚠️ macOS AppleScript 导航执行失败: {e}{Style.RESET_ALL}")
        return False

    if result.returncode != 0:
        detail = (result.stderr or result.stdout or "").strip()
        print(f"{Fore.YELLOW}⚠️ macOS AppleScript 导航失败: {detail}{Style.RESET_ALL}")
        return False

    print(f"{Fore.CYAN}🧭 通过 macOS 前台窗口标签页直接打开目标 URL{Style.RESET_ALL}")
    return True

def _rebind_page_to_loaded_url(browser_page, url, timeout_s=8.0):
    """Rebind CDP page object to the real tab that loaded target URL."""
    browser = getattr(browser_page, "browser", None)
    if browser is None:
        return browser_page

    parsed = urlparse(url)
    url_candidates = [url]
    if parsed.scheme and parsed.netloc:
        url_candidates.append(f"{parsed.scheme}://{parsed.netloc}")
        url_candidates.append(parsed.netloc)

    end_at = time.time() + max(0.5, timeout_s)
    while time.time() < end_at:
        for candidate in url_candidates:
            try:
                tabs = browser._get_tabs(url=candidate, tab_type="page", mix=False)
            except Exception:
                tabs = []
            if tabs:
                tab = tabs[0]
                try:
                    setter = getattr(tab, "set", None)
                    activate = getattr(setter, "activate", None)
                    if callable(activate):
                        activate()
                except Exception:
                    pass
                current_url = getattr(tab, "url", "") or candidate
                print(f"{Fore.CYAN}🔗 CDP 已重新绑定到真实标签页: {current_url}{Style.RESET_ALL}")
                return tab
        time.sleep(0.25)

    print(f"{Fore.YELLOW}⚠️ 未找到已加载目标 URL 的真实标签页，继续沿用当前页对象{Style.RESET_ALL}")
    return browser_page

def _open_primary_flow_page(browser_page, url):
    """
    Open URL in visible primary page. On macOS + incognito, prefer AppleScript
    front-tab navigation to avoid hidden target mismatch.
    """
    browser = getattr(browser_page, "browser", None)
    if sys.platform == "darwin" and browser is not None:
        is_incognito = False
        try:
            is_incognito = bool(getattr(browser, "states", None) and browser.states.is_incognito)
        except Exception:
            is_incognito = False

        if is_incognito and _navigate_front_window_via_macos_applescript(url):
            time.sleep(1.2)
            return _rebind_page_to_loaded_url(browser_page, url)

    browser_page.get(url)
    return browser_page

def simulate_human_input(page, url, config, translator=None):
    """Visit URL"""
    if translator:
        print(f"{Fore.CYAN}🚀 {translator.get('register.visiting_url')}: {url}{Style.RESET_ALL}")
    
    # First visit blank page
    page.get('about:blank')
    time.sleep(get_random_wait_time(config, 'page_load_wait'))
    
    # Visit target page
    page = _open_primary_flow_page(page, url)
    time.sleep(get_random_wait_time(config, 'page_load_wait'))
    return page

def fill_signup_form(page, first_name, last_name, email, config, translator=None):
    """Fill signup form"""
    try:
        if translator:
            print(f"{Fore.CYAN}📧 {translator.get('register.filling_form')}{Style.RESET_ALL}")
        else:
            print("\n正在填写注册表单...")
        
        # 检查页面URL状态
        current_url = page.url
        
        # 检查是否在正确的注册页面
        if "authenticator.cursor.sh/sign-up" in current_url:
            # 检查URL是否包含必要的参数
            if "client_id=" not in current_url or "redirect_uri=" not in current_url:
                print(f"{Fore.YELLOW}⚠️ [填写表单前] 注册页面URL缺少必要参数（可能遇到Cloudflare验证）{Style.RESET_ALL}")
                print(f"{Fore.YELLOW}📋 当前URL: {current_url}{Style.RESET_ALL}")
                print(f"{Fore.CYAN}💡 请手动完成Cloudflare验证{Style.RESET_ALL}")
                
                # 等待CF验证完成
                wait_time = 0
                max_wait = 60
                while wait_time < max_wait:
                    time.sleep(2)
                    wait_time += 2
                    current_url = page.url
                    
                    if "client_id=" in current_url and "redirect_uri=" in current_url:
                        print(f"{Fore.GREEN}✅ 页面已跳转到正确的注册URL{Style.RESET_ALL}")
                        break
                    
                    if wait_time % 10 == 0:
                        print(f"{Fore.CYAN}⏳ 等待CF验证完成... ({wait_time}/{max_wait}秒){Style.RESET_ALL}")
                
                if wait_time >= max_wait:
                    print(f"{Fore.RED}❌ 等待超时{Style.RESET_ALL}")
                    return False
        
        # 检查是否在老验证页面
        elif "authenticate.cursor.sh/user_management/initiate_login" in current_url:
            print(f"{Fore.YELLOW}⚠️ [填写表单前] 检测到老验证页面{Style.RESET_ALL}")
            
            max_retry = 3
            for retry in range(max_retry):
                sign_up_url = "https://authenticator.cursor.sh/sign-up"
                page.get(sign_up_url)
                print(f"{Fore.CYAN}🔄 已重新跳转到: {sign_up_url} (第{retry+1}次){Style.RESET_ALL}")
                time.sleep(get_random_wait_time(config, 'page_load_wait'))
                
                current_url = page.url
                if "authenticate.cursor.sh/user_management/initiate_login" not in current_url:
                    break
            
            if "authenticate.cursor.sh/user_management/initiate_login" in page.url:
                print(f"{Fore.RED}❌ 多次重试后仍在老验证页面{Style.RESET_ALL}")
                return False
        
        # Fill first name
        first_name_input = page.ele("@name=first_name")
        if first_name_input:
            first_name_input.input(first_name)
            time.sleep(get_random_wait_time(config, 'input_wait'))
        
        # Fill last name
        last_name_input = page.ele("@name=last_name")
        if last_name_input:
            last_name_input.input(last_name)
            time.sleep(get_random_wait_time(config, 'input_wait'))
        
        # Fill email
        email_input = page.ele("@name=email")
        if email_input:
            email_input.input(email)
            time.sleep(get_random_wait_time(config, 'input_wait'))
        
        # Click submit button
        submit_button = page.ele("@type=submit")
        if submit_button:
            submit_button.click()
            time.sleep(get_random_wait_time(config, 'submit_wait'))
            
        if translator:
            print(f"{Fore.GREEN}✅ {translator.get('register.form_success')}{Style.RESET_ALL}")
        else:
            print("Form filled successfully")
        return True
        
    except Exception as e:
        if translator:
            print(f"{Fore.RED}❌ {translator.get('register.form_error', error=str(e))}{Style.RESET_ALL}")
        else:
            print(f"Error filling form: {e}")
        return False

def get_user_documents_path():
    """Get user Documents folder path"""
    if sys.platform == "win32":
        try:
            import winreg
            with winreg.OpenKey(winreg.HKEY_CURRENT_USER, "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Shell Folders") as key:
                documents_path, _ = winreg.QueryValueEx(key, "Personal")
                return documents_path
        except Exception as e:
            # fallback
            return os.path.join(os.path.expanduser("~"), "Documents")
    elif sys.platform == "darwin":
        return os.path.join(os.path.expanduser("~"), "Documents")
    else:  # Linux
        # Get actual user's home directory
        sudo_user = os.environ.get('SUDO_USER')
        if sudo_user:
            return os.path.join("/home", sudo_user, "Documents")
        return os.path.join(os.path.expanduser("~"), "Documents")

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

def setup_driver(translator=None, use_incognito=True, custom_config=None):
    # 调试日志
    print(f"🔍 [DEBUG] setup_driver 调用:")
    print(f"  - use_incognito 参数: {use_incognito}")
    print(f"  - use_incognito 类型: {type(use_incognito)}")
    print(f"  - custom_config: {custom_config}")
    """Setup browser driver"""
    global _chrome_process_ids
    
    try:
        # Get config
        config = get_config(translator)
        
        # Get browser type and path
        browser_type = config.get('Browser', 'default_browser', fallback='chrome')
        browser_path = config.get('Browser', f'{browser_type}_path', fallback=utils_get_default_browser_path(browser_type))
        
        # 检查是否有自定义浏览器路径（从前端传来）
        if custom_config and 'custom_browser_path' in custom_config:
            custom_browser_path = custom_config['custom_browser_path']
            if custom_browser_path and os.path.exists(custom_browser_path):
                browser_path = custom_browser_path
                print(f"{Fore.CYAN}🌐 使用自定义浏览器路径: {browser_path}{Style.RESET_ALL}")
            else:
                print(f"{Fore.YELLOW}⚠️ 自定义浏览器路径无效，使用默认路径{Style.RESET_ALL}")
        
        if not browser_path or not os.path.exists(browser_path):
            if translator:
                print(f"{Fore.YELLOW}⚠️ {browser_type} {translator.get('register.browser_path_invalid')}{Style.RESET_ALL}")
            browser_path = utils_get_default_browser_path(browser_type)

        # For backward compatibility, also check Chrome path
        if browser_type == 'chrome':
            chrome_path = config.get('Chrome', 'chromepath', fallback=None)
            if chrome_path and os.path.exists(chrome_path):
                browser_path = chrome_path

        # Set browser options
        co = ChromiumOptions()

        # Set browser path
        co.set_browser_path(browser_path)

        # Use incognito mode (configurable)
        print(f"🔍 [DEBUG] 无痕模式检查:")
        print(f"  - use_incognito 值: {use_incognito}")
        print(f"  - use_incognito 类型: {type(use_incognito)}")
        print(f"  - 条件判断结果: {bool(use_incognito)}")

        if use_incognito:
            print("✅ [DEBUG] 启用无痕模式 - 添加 --incognito 参数")
            co.set_argument("--incognito")
        else:
            print("❌ [DEBUG] 禁用无痕模式 - 不添加 --incognito 参数")

        if sys.platform == "linux":
            # Set Linux specific options
            co.set_argument("--no-sandbox")

        # Set random port
        co.auto_port()

        # Use headless mode (must be set to False, simulate human operation)
        co.headless(False)

        # Set window size (width, height)
        co.set_argument("--window-size=1280,720")  # 可以修改为你想要的宽度和高度

        # Configure proxy settings
        # 优先使用 custom_config 中的代理配置，如果没有则使用配置文件中的设置
        if custom_config and 'proxy' in custom_config:
            proxy_config = custom_config['proxy']
            proxy_enabled = proxy_config.get('enabled', False)
            
            if proxy_enabled:
                proxy_type = proxy_config.get('proxy_type', 'http').lower()
                
                if proxy_type in ['http', 'https']:
                    http_proxy = proxy_config.get('http_proxy', '')
                    
                    if http_proxy:
                        co.set_argument(f"--proxy-server=http://{http_proxy}")
                        if translator:
                            print(f"{Fore.CYAN}🌐 {translator.get('register.proxy_enabled', proxy=http_proxy)}{Style.RESET_ALL}")
                        else:
                            print(f"{Fore.CYAN}🌐 代理已启用 (来自前端配置): http://{http_proxy}{Style.RESET_ALL}")
                            
                elif proxy_type.startswith('socks'):
                    socks_proxy = proxy_config.get('socks_proxy', '')
                    if socks_proxy:
                        co.set_argument(f"--proxy-server=socks5://{socks_proxy}")
                        if translator:
                            print(f"{Fore.CYAN}🌐 {translator.get('register.socks_proxy_enabled', proxy=socks_proxy)}{Style.RESET_ALL}")
                        else:
                            print(f"{Fore.CYAN}🌐 SOCKS代理已启用 (来自前端配置): socks5://{socks_proxy}{Style.RESET_ALL}")
                
                # Set proxy bypass list
                no_proxy = proxy_config.get('no_proxy', '')
                if no_proxy:
                    co.set_argument(f"--proxy-bypass-list={no_proxy}")
            else:
                if translator:
                    print(f"{Fore.YELLOW}⚠️ {translator.get('register.proxy_disabled')}{Style.RESET_ALL}")
                else:
                    print(f"{Fore.YELLOW}⚠️ 代理已禁用 (来自前端配置){Style.RESET_ALL}")
        else:
            # 回退到配置文件中的代理设置
            proxy_enabled = config.get('Proxy', 'enabled', fallback='False').lower() in ('true', 'yes', '1', 'on')
            if proxy_enabled:
                proxy_type = config.get('Proxy', 'proxy_type', fallback='http').lower()
                
                if proxy_type in ['http', 'https']:
                    http_proxy = config.get('Proxy', 'http_proxy', fallback='')
                    
                    if http_proxy:
                        co.set_argument(f"--proxy-server=http://{http_proxy}")
                        if translator:
                            print(f"{Fore.CYAN}🌐 {translator.get('register.proxy_enabled', proxy=http_proxy)}{Style.RESET_ALL}")
                        else:
                            print(f"{Fore.CYAN}🌐 代理已启用 (来自配置文件): http://{http_proxy}{Style.RESET_ALL}")
                            
                elif proxy_type.startswith('socks'):
                    socks_proxy = config.get('Proxy', 'socks_proxy', fallback='')
                    if socks_proxy:
                        co.set_argument(f"--proxy-server=socks5://{socks_proxy}")
                        if translator:
                            print(f"{Fore.CYAN}🌐 {translator.get('register.socks_proxy_enabled', proxy=socks_proxy)}{Style.RESET_ALL}")
                        else:
                            print(f"{Fore.CYAN}🌐 SOCKS代理已启用 (来自配置文件): socks5://{socks_proxy}{Style.RESET_ALL}")
                
                # Set proxy bypass list
                no_proxy = config.get('Proxy', 'no_proxy', fallback='')
                if no_proxy:
                    co.set_argument(f"--proxy-bypass-list={no_proxy}")
            else:
                if translator:
                    print(f"{Fore.YELLOW}⚠️ {translator.get('register.proxy_disabled')}{Style.RESET_ALL}")
                else:
                    print(f"{Fore.YELLOW}⚠️ 代理已禁用 (来自配置文件){Style.RESET_ALL}")

        # 可选：使用app模式减少浏览器UI（取消注释下面的行来启用）
        # co.set_argument("--app=https://authenticator.cursor.sh/sign-up")

        # 可选：使用kiosk模式（全屏，无法调整窗口大小，取消注释下面的行来启用）
        # co.set_argument("--kiosk")

        # Log browser info
        if translator:
            print(f"{Fore.CYAN}🌐 {translator.get('register.using_browser', browser=browser_type, path=browser_path)}{Style.RESET_ALL}")
        
        try:
            # Load extension
            extension_path = os.path.join(os.getcwd(), "turnstilePatch")
            if os.path.exists(extension_path):
                if use_incognito:
                    co.set_argument("--allow-extensions-in-incognito")
                co.add_extension(extension_path)
        except Exception as e:
            if translator:
                print(f"{Fore.RED}❌ {translator.get('register.extension_load_error', error=str(e))}{Style.RESET_ALL}")
            else:
                print(f"Error loading extension: {e}")
        
        if translator:
            print(f"{Fore.CYAN}🚀 {translator.get('register.starting_browser')}{Style.RESET_ALL}")
        else:
            print("Starting browser...")
        
        # Record Chrome processes before launching
        before_pids = []
        try:
            import psutil
            browser_process_names = {
                'chrome': ['chrome', 'chromium'],
                'edge': ['msedge', 'edge'],
                'firefox': ['firefox'],
                'brave': ['brave', 'brave-browser']
            }
            process_names = browser_process_names.get(browser_type, ['chrome'])
            before_pids = [p.pid for p in psutil.process_iter() if any(name in p.name().lower() for name in process_names)]
        except:
            pass
            
        # Launch browser
        page = ChromiumPage(co)
        
        # Wait a moment for browser to fully launch
        time.sleep(1)
        
        # Record browser processes after launching and find new ones
        try:
            import psutil
            process_names = browser_process_names.get(browser_type, ['chrome'])
            after_pids = [p.pid for p in psutil.process_iter() if any(name in p.name().lower() for name in process_names)]
            # Find new browser processes
            new_pids = [pid for pid in after_pids if pid not in before_pids]
            _chrome_process_ids.extend(new_pids)
            
            if _chrome_process_ids:
                if translator:
                    print(
                        f"{translator.get('register.tracking_processes', count=len(_chrome_process_ids), browser=browser_type)}"
                    )
                else:
                    print(f"Tracking {len(_chrome_process_ids)} {browser_type} process(es) launched by this script.")
            else:
                if translator:
                    msg = translator.get('register.no_new_processes_detected', browser=browser_type)
                else:
                    msg = f"No new {browser_type} processes detected."
                print(f"{Fore.YELLOW}Warning: {msg}{Style.RESET_ALL}")
        except Exception as e:
            if translator:
                print(
                    f"{translator.get('register.could_not_track_processes', browser=browser_type, error=str(e))}"
                )
            else:
                print(f"Could not track {browser_type} processes: {e}")
            
        return config, page

    except Exception as e:
        if translator:
            print(f"{Fore.RED}❌ {translator.get('register.browser_setup_error', error=str(e))}{Style.RESET_ALL}")
        else:
            print(f"Error setting up browser: {e}")
        raise

def handle_turnstile(page, config, translator=None):
    """Handle Turnstile verification"""
    try:
        if translator:
            print(f"{Fore.CYAN}🔄 {translator.get('register.handling_turnstile')}{Style.RESET_ALL}")
        else:
            print("\nHandling Turnstile verification...")
        
        # from config
        turnstile_time = float(config.get('Turnstile', 'handle_turnstile_time', fallback='2'))
        random_time_str = config.get('Turnstile', 'handle_turnstile_random_time', fallback='1-3')
        
        # Parse random time range
        try:
            min_time, max_time = map(float, random_time_str.split('-'))
        except:
            min_time, max_time = 1, 3  # Default value
        
        max_retries = 2
        retry_count = 0

        while retry_count < max_retries:
            retry_count += 1
            if translator:
                print(f"{Fore.CYAN}🔄 {translator.get('register.retry_verification', attempt=retry_count)}{Style.RESET_ALL}")
            else:
                print(f"Attempt {retry_count} of verification...")

            try:
                # Try to reset turnstile
                page.run_js("try { turnstile.reset() } catch(e) { }")
                time.sleep(turnstile_time)  # from config

                # Locate verification box element
                challenge_check = (
                    page.ele("@id=cf-turnstile", timeout=2)
                    .child()
                    .shadow_root.ele("tag:iframe")
                    .ele("tag:body")
                    .sr("tag:input")
                )

                if challenge_check:
                    if translator:
                        print(f"{Fore.CYAN}🔄 {translator.get('register.detect_turnstile')}{Style.RESET_ALL}")
                    else:
                        print("Detected verification box...")
                    
                    # from config
                    time.sleep(random.uniform(min_time, max_time))
                    challenge_check.click()
                    time.sleep(turnstile_time)  # from config

                    # check verification result
                    if check_verification_success(page, translator):
                        if translator:
                            print(f"{Fore.GREEN}✅ {translator.get('register.verification_success')}{Style.RESET_ALL}")
                        else:
                            print("Verification successful!")
                        return True

            except Exception as e:
                if translator:
                    print(f"{Fore.RED}❌ {translator.get('register.verification_failed')}{Style.RESET_ALL}")
                else:
                    print(f"Verification attempt failed: {e}")

            # Check if verification has been successful
            if check_verification_success(page, translator):
                if translator:
                    print(f"{Fore.GREEN}✅ {translator.get('register.verification_success')}{Style.RESET_ALL}")
                else:
                    print("Verification successful!")
                return True

            time.sleep(random.uniform(min_time, max_time))

        if translator:
            print(f"{Fore.RED}❌ {translator.get('register.verification_failed')}{Style.RESET_ALL}")
        else:
            print("Exceeded maximum retry attempts")
        return False

    except Exception as e:
        if translator:
            print(f"{Fore.RED}❌ {translator.get('register.verification_error', error=str(e))}{Style.RESET_ALL}")
        else:
            print(f"Error in verification process: {e}")
        return False

def check_verification_success(page, translator=None):
    """Check if verification is successful"""
    try:
        # Check if there is a subsequent form element, indicating verification has passed
        if (page.ele("@name=password", timeout=0.5) or 
            page.ele("@name=email", timeout=0.5) or
            page.ele("@data-index=0", timeout=0.5) or
            page.ele("Account Settings", timeout=0.5)):
            return True
        
        # Check if there is an error message
        error_messages = [
            'xpath://div[contains(text(), "Can\'t verify the user is human")]',
            'xpath://div[contains(text(), "Error: 600010")]',
            'xpath://div[contains(text(), "Please try again")]'
        ]
        
        for error_xpath in error_messages:
            if page.ele(error_xpath):
                return False
            
        return False
    except:
        return False

def generate_password(length=12):
    """Generate random password"""
    chars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%^&*"
    return ''.join(random.choices(chars, k=length))

def fill_password(page, password: str, config, translator=None):
    """
    Fill password form
    """
    try:
        print(f"{Fore.CYAN}🔑 {translator.get('register.setting_password') if translator else 'Setting password'}{Style.RESET_ALL}")
        
        # Fill password
        password_input = page.ele("@name=password")
        print(f"{Fore.CYAN}🔑 {translator.get('register.setting_on_password')}: {password}{Style.RESET_ALL}")
        if password_input:
            password_input.input(password)

        # Click submit button
        submit_button = page.ele("@type=submit")
        if submit_button:
            submit_button.click()
            time.sleep(get_random_wait_time(config, 'submit_wait'))
            
        print(f"{Fore.GREEN}✅ {translator.get('register.password_submitted') if translator else 'Password submitted'}{Style.RESET_ALL}")
        
        return True
        
    except Exception as e:
        print(f"{Fore.RED}❌ {translator.get('register.password_error', error=str(e)) if translator else f'Error setting password: {str(e)}'}{Style.RESET_ALL}")

        return False

def wait_for_phone_verification(browser_tab, translator=None, controller=None):
    """Check if phone verification page appears and wait for user to complete it"""
    try:
        print(f"{Fore.CYAN}📱 等待并检查是否出现手机号验证页面...{Style.RESET_ALL}")
        
        # Wait a bit to let Cursor decide if phone verification is needed
        # If on dashboard and phone verification is required, Cursor will redirect
        time.sleep(5)
        
        # Check current URL after waiting
        current_url = browser_tab.url
        print(f"{Fore.CYAN}📍 当前URL: {current_url}{Style.RESET_ALL}")
        
        if "radar-challenge/" in current_url:
            print(f"\n{Fore.YELLOW}{'='*60}{Style.RESET_ALL}")
            print(f"{Fore.YELLOW}⚠️  检测到手机号验证页面！{Style.RESET_ALL}")
            print(f"{Fore.YELLOW}📱 当前URL: {current_url}{Style.RESET_ALL}")
            print(f"{Fore.YELLOW}⏳ 请在浏览器中完成手机号验证...{Style.RESET_ALL}")
            print(f"{Fore.YELLOW}{'='*60}{Style.RESET_ALL}\n")
            
            # Wait for user to complete phone verification
            wait_count = 0
            while True:
                time.sleep(5)  # Check every 5 seconds
                wait_count += 1
                
                try:
                    current_url = browser_tab.url
                    
                    # If URL no longer contains radar-challenge/, verification is complete
                    if "radar-challenge/" not in current_url:
                        print(f"{Fore.GREEN}✅ 手机号验证已完成！{Style.RESET_ALL}")
                        print(f"{Fore.CYAN}📍 当前URL: {current_url}{Style.RESET_ALL}")
                        return True
                    
                    # Print waiting status every 30 seconds
                    if wait_count % 6 == 0:
                        elapsed_time = wait_count * 5
                        print(f"{Fore.CYAN}⏳ 等待手机号验证中... (已等待 {elapsed_time} 秒){Style.RESET_ALL}")
                        print(f"{Fore.CYAN}📍 当前URL: {current_url}{Style.RESET_ALL}")
                        
                except Exception as e:
                    print(f"{Fore.RED}❌ 检查URL时出错: {str(e)}{Style.RESET_ALL}")
                    # If error occurs, assume browser might be closed or other issues
                    return False
        else:
            print(f"{Fore.GREEN}✅ 未检测到手机号验证页面，继续后续流程{Style.RESET_ALL}")
            # 检查是否勾选了自动绑定银行卡
            if controller and hasattr(controller, 'enable_bank_card_binding'):
                if not controller.enable_bank_card_binding:
                    print(f"{Fore.CYAN}💳 未勾选自动绑定银行卡，直接完成注册流程{Style.RESET_ALL}")
                    return "skip_settings"  # 返回特殊值，表示跳过访问settings页面
            return True
            
    except Exception as e:
        print(f"{Fore.RED}❌ 检查手机号验证页面时出错: {str(e)}{Style.RESET_ALL}")
        return True  # If error occurs, continue with normal flow

def handle_verification_code(browser_tab, email_tab, controller, config, translator=None):
    """Handle verification code"""
    try:
        if translator:
            print(f"\n{Fore.CYAN}🔄 {translator.get('register.waiting_for_verification_code')}{Style.RESET_ALL}")
            
        # Check if using manual input verification code
        if hasattr(controller, 'get_verification_code') and email_tab is None:  # Manual mode
            verification_code = controller.get_verification_code()
            if verification_code:
                # Fill verification code in registration page
                for i, digit in enumerate(verification_code):
                    browser_tab.ele(f"@data-index={i}").input(digit)
                    time.sleep(get_random_wait_time(config, 'verification_code_input'))
                
                print(f"{translator.get('register.verification_success')}")
                time.sleep(get_random_wait_time(config, 'verification_success_wait'))
                
                # Handle last Turnstile verification
                if handle_turnstile(browser_tab, config, translator):
                    if translator:
                        print(f"{Fore.GREEN}✅ {translator.get('register.verification_success')}{Style.RESET_ALL}")
                    time.sleep(get_random_wait_time(config, 'verification_retry_wait'))
                    
                    # Check for phone verification page before visiting settings
                    phone_verification_result = wait_for_phone_verification(browser_tab, translator, controller)
                    if phone_verification_result == False:
                        print(f"{Fore.RED}❌ 手机号验证检查失败{Style.RESET_ALL}")
                        return False, None
                    
                    # 如果未勾选自动绑定银行卡，直接返回完成
                    if phone_verification_result == "skip_settings":
                        print(f"{Fore.GREEN}✅ 注册流程已完成，跳过访问settings页面{Style.RESET_ALL}")
                        return True, browser_tab
                    
                    # Visit settings page
                    print(f"{Fore.CYAN}🔑 {translator.get('register.visiting_url')}: https://www.cursor.com/settings{Style.RESET_ALL}")
                    browser_tab.get("https://www.cursor.com/settings")
                    time.sleep(get_random_wait_time(config, 'settings_page_load_wait'))
                    return True, browser_tab
                    
                return False, None
                
        # Automatic verification code logic
        elif email_tab:
            print(f"{Fore.CYAN}🔄 {translator.get('register.waiting_for_verification_code')}{Style.RESET_ALL}")
            time.sleep(get_random_wait_time(config, 'email_check_initial_wait'))

            # Use existing email_tab to refresh email
            email_tab.refresh_inbox()
            time.sleep(get_random_wait_time(config, 'email_refresh_wait'))

            # Check if there is a verification code email
            if email_tab.check_for_cursor_email():
                verification_code = email_tab.get_verification_code()
                if verification_code:
                    # Fill verification code in registration page
                    for i, digit in enumerate(verification_code):
                        browser_tab.ele(f"@data-index={i}").input(digit)
                        time.sleep(get_random_wait_time(config, 'verification_code_input'))
                    
                    if translator:
                        print(f"{Fore.GREEN}✅ {translator.get('register.verification_success')}{Style.RESET_ALL}")
                    time.sleep(get_random_wait_time(config, 'verification_success_wait'))
                    
                    # Handle last Turnstile verification
                    if handle_turnstile(browser_tab, config, translator):
                        if translator:
                            print(f"{Fore.GREEN}✅ {translator.get('register.verification_success')}{Style.RESET_ALL}")
                        time.sleep(get_random_wait_time(config, 'verification_retry_wait'))
                        
                        # Check for phone verification page before visiting settings
                        phone_verification_result = wait_for_phone_verification(browser_tab, translator, controller)
                        if phone_verification_result == False:
                            print(f"{Fore.RED}❌ 手机号验证检查失败{Style.RESET_ALL}")
                            return False, None
                        
                        # 如果未勾选自动绑定银行卡，直接返回完成
                        if phone_verification_result == "skip_settings":
                            print(f"{Fore.GREEN}✅ 注册流程已完成，跳过访问settings页面{Style.RESET_ALL}")
                            return True, browser_tab
                        
                        # Visit settings page
                        if translator:
                            print(f"{Fore.CYAN}🔑 {translator.get('register.visiting_url')}: https://www.cursor.com/settings{Style.RESET_ALL}")
                        browser_tab.get("https://www.cursor.com/settings")
                        time.sleep(get_random_wait_time(config, 'settings_page_load_wait'))
                        return True, browser_tab
                        
                    else:
                        if translator:
                            print(f"{Fore.RED}❌ {translator.get('register.verification_failed')}{Style.RESET_ALL}")
                        else:
                            print("最后一次验证失败")
                        return False, None
                        
            # Get verification code, set timeout
            verification_code = None
            max_attempts = 20
            retry_interval = get_random_wait_time(config, 'retry_interval')  # Use get_random_wait_time
            start_time = time.time()
            timeout = float(config.get('Timing', 'max_timeout', fallback='160'))  # This can be kept unchanged because it is a fixed value

            if translator:
                print(f"{Fore.CYAN}{translator.get('register.start_getting_verification_code')}{Style.RESET_ALL}")
            
            for attempt in range(max_attempts):
                # Check if timeout
                if time.time() - start_time > timeout:
                    if translator:
                        print(f"{Fore.RED}❌ {translator.get('register.verification_timeout')}{Style.RESET_ALL}")
                    break
                    
                verification_code = controller.get_verification_code()
                if verification_code:
                    if translator:
                        print(f"{Fore.GREEN}✅ {translator.get('register.verification_success')}{Style.RESET_ALL}")
                    break
                    
                remaining_time = int(timeout - (time.time() - start_time))
                if translator:
                    print(f"{Fore.CYAN}{translator.get('register.try_get_code', attempt=attempt + 1, time=remaining_time)}{Style.RESET_ALL}")
                
                # Refresh email
                email_tab.refresh_inbox()
                time.sleep(retry_interval)  # Use get_random_wait_time
            
            if verification_code:
                # Fill verification code in registration page
                for i, digit in enumerate(verification_code):
                    browser_tab.ele(f"@data-index={i}").input(digit)
                    time.sleep(get_random_wait_time(config, 'verification_code_input'))
                
                if translator:
                    print(f"{Fore.GREEN}✅ {translator.get('register.verification_success')}{Style.RESET_ALL}")
                time.sleep(get_random_wait_time(config, 'verification_success_wait'))
                
                # Handle last Turnstile verification
                if handle_turnstile(browser_tab, config, translator):
                    if translator:
                        print(f"{Fore.GREEN}✅ {translator.get('register.verification_success')}{Style.RESET_ALL}")
                    time.sleep(get_random_wait_time(config, 'verification_retry_wait'))
                    
                    # Check for phone verification page before visiting settings
                    phone_verification_result = wait_for_phone_verification(browser_tab, translator, controller)
                    if phone_verification_result == False:
                        print(f"{Fore.RED}❌ 手机号验证检查失败{Style.RESET_ALL}")
                        return False, None
                    
                    # 如果未勾选自动绑定银行卡，直接返回完成
                    if phone_verification_result == "skip_settings":
                        print(f"{Fore.GREEN}✅ 注册流程已完成，跳过访问settings页面{Style.RESET_ALL}")
                        return True, browser_tab
                    
                    # Visit settings page
                    if translator:
                        print(f"{Fore.CYAN}{translator.get('register.visiting_url')}: https://www.cursor.com/settings{Style.RESET_ALL}")
                    browser_tab.get("https://www.cursor.com/settings")
                    time.sleep(get_random_wait_time(config, 'settings_page_load_wait'))
                    
                    # Return success directly, let cursor_register.py handle account information acquisition
                    return True, browser_tab
                    
                else:
                    if translator:
                        print(f"{Fore.RED}❌ {translator.get('register.verification_failed')}{Style.RESET_ALL}")
                    return False, None
                
            return False, None
            
    except Exception as e:
        if translator:
            print(f"{Fore.RED}❌ {translator.get('register.verification_error', error=str(e))}{Style.RESET_ALL}")
        return False, None

def handle_sign_in(browser_tab, email, password, translator=None):
    """Handle login process"""
    try:
        # Check if on login page
        sign_in_header = browser_tab.ele('xpath://h1[contains(text(), "Sign in")]')
        if not sign_in_header:
            return True  # If not on login page, it means login is successful
            
        print(f"{Fore.CYAN}检测到登录页面，开始登录...{Style.RESET_ALL}")
        
        # Fill email
        email_input = browser_tab.ele('@name=email')
        if email_input:
            email_input.input(email)
            time.sleep(1)
            
            # Click Continue
            continue_button = browser_tab.ele('xpath://button[contains(@class, "BrandedButton") and text()="Continue"]')
            if continue_button:
                continue_button.click()
                time.sleep(2)
                
                # Handle Turnstile verification
                if handle_turnstile(browser_tab, translator):
                    # Fill password
                    password_input = browser_tab.ele('@name=password')
                    if password_input:
                        password_input.input(password)
                        time.sleep(1)
                        
                        # Click Sign in
                        sign_in_button = browser_tab.ele('xpath://button[@name="intent" and @value="password"]')
                        if sign_in_button:
                            sign_in_button.click()
                            time.sleep(2)
                            
                            # Handle last Turnstile verification
                            if handle_turnstile(browser_tab, translator):
                                print(f"{Fore.GREEN}Login successful!{Style.RESET_ALL}")
                                time.sleep(3)
                                return True
                                
        print(f"{Fore.RED}Login failed{Style.RESET_ALL}")
        return False
        
    except Exception as e:
        print(f"{Fore.RED}Login process error: {str(e)}{Style.RESET_ALL}")
        return False

def main(email=None, password=None, first_name=None, last_name=None, email_tab=None, controller=None, translator=None, use_incognito=True, skip_phone_verification=False, custom_config=None):
    # 调试日志
    print(f"🔍 [DEBUG] new_signup.main 调用:")
    print(f"  - use_incognito 参数: {use_incognito}")
    print(f"  - use_incognito 类型: {type(use_incognito)}")
    print(f"  - skip_phone_verification: {skip_phone_verification}")
    print(f"  - custom_config: {custom_config}")
    """Main function, can receive account information, email tab, and translator"""
    global _translator
    global _chrome_process_ids
    _translator = translator  # Save to global variable
    _chrome_process_ids = []  # Reset the process IDs list
    
    signal.signal(signal.SIGINT, signal_handler)
    signal.signal(signal.SIGTERM, signal_handler)
    
    page = None
    success = False
    try:
        config, page = setup_driver(translator, use_incognito, custom_config)
        if translator:
            print(f"{Fore.CYAN}🚀 {translator.get('register.browser_started')}{Style.RESET_ALL}")
        
        # Visit registration page - change URL if skip_phone_verification is enabled
        if skip_phone_verification:
            url = "https://authenticator.cursor.sh/"
            print(f"{Fore.CYAN}🔄 使用跳过手机号验证模式，访问: {url}{Style.RESET_ALL}")
        else:
            url = "https://authenticator.cursor.sh/sign-up"
        
        # Visit page
        page = simulate_human_input(page, url, config, translator)
        if translator:
            print(f"{Fore.CYAN}🔄 {translator.get('register.waiting_for_page_load')}{Style.RESET_ALL}")
        time.sleep(get_random_wait_time(config, 'page_load_wait'))
        
        # 检查页面URL状态
        current_url = page.url
        print(f"{Fore.CYAN}🔍 页面加载完成，当前URL: {current_url}{Style.RESET_ALL}")
        
        # 检查URL类型
        max_retry_for_old_page = 3
        retry_count = 0
        
        while retry_count < max_retry_for_old_page:
            current_url = page.url
            
            # 情况1: 老验证页面 - 需要重试
            if "authenticate.cursor.sh/user_management/initiate_login" in current_url:
                retry_count += 1
                print(f"{Fore.YELLOW}⚠️ 检测到跳转到老验证页面，尝试重新跳转到注册页面 (第{retry_count}次重试){Style.RESET_ALL}")
                
                # 重新跳转到注册页面
                sign_up_url = "https://authenticator.cursor.sh/sign-up"
                page.get(sign_up_url)
                print(f"{Fore.CYAN}🔄 已重新跳转到: {sign_up_url}{Style.RESET_ALL}")
                time.sleep(get_random_wait_time(config, 'page_load_wait'))
            
            # 情况2: 注册页面但没有必要参数 - 可能遇到CF验证
            elif "authenticator.cursor.sh/sign-up" in current_url:
                # 检查URL是否包含必要的参数（client_id, redirect_uri等）
                if "client_id=" not in current_url or "redirect_uri=" not in current_url:
                    print(f"{Fore.YELLOW}⚠️ 检测到注册页面URL缺少必要参数（可能遇到Cloudflare验证）{Style.RESET_ALL}")
                    print(f"{Fore.YELLOW}📋 当前URL: {current_url}{Style.RESET_ALL}")
                    print(f"{Fore.CYAN}💡 请手动完成Cloudflare验证后，页面会自动跳转到正确的注册页面{Style.RESET_ALL}")
                    
                    # 等待用户手动完成CF验证
                    print(f"{Fore.CYAN}⏳ 等待页面自动跳转...（最多等待60秒）{Style.RESET_ALL}")
                    
                    # 轮询检查URL是否已经包含参数
                    wait_time = 0
                    max_wait = 60
                    while wait_time < max_wait:
                        time.sleep(2)
                        wait_time += 2
                        current_url = page.url
                        
                        # 检查是否已经跳转到正确的URL
                        if "client_id=" in current_url and "redirect_uri=" in current_url:
                            print(f"{Fore.GREEN}✅ 检测到页面已跳转到正确的注册URL{Style.RESET_ALL}")
                            break
                        
                        if wait_time % 10 == 0:
                            print(f"{Fore.CYAN}⏳ 继续等待... ({wait_time}/{max_wait}秒){Style.RESET_ALL}")
                    
                    if wait_time >= max_wait:
                        print(f"{Fore.RED}❌ 等待超时，页面未跳转到正确的注册URL{Style.RESET_ALL}")
                        return False, None
                    
                    # 已经跳转到正确URL，跳出循环
                    break
                else:
                    # URL包含必要参数，正确
                    print(f"{Fore.GREEN}✅ 页面URL正确，包含必要的注册参数{Style.RESET_ALL}")
                    break
            
            # 情况3: 其他authenticator页面
            elif "authenticator.cursor.sh" in current_url:
                print(f"{Fore.GREEN}✅ 页面URL正确{Style.RESET_ALL}")
                break
            else:
                # 未知页面
                print(f"{Fore.YELLOW}⚠️ 检测到未知页面: {current_url}{Style.RESET_ALL}")
                break
        
        if retry_count >= max_retry_for_old_page:
            print(f"{Fore.RED}❌ 多次重试后仍跳转到老验证页面，注册失败{Style.RESET_ALL}")
            return False, None
        
        # If account information is not provided, generate random information
        if not all([email, password, first_name, last_name]):
            first_name = ''.join(random.choices('abcdefghijklmnopqrstuvwxyz', k=6)).capitalize()
            last_name = ''.join(random.choices('abcdefghijklmnopqrstuvwxyz', k=6)).capitalize()
            email = f"{first_name.lower()}{random.randint(100,999)}@example.com"
            password = generate_password()
            
            # Save account information
            with open('test_accounts.txt', 'a', encoding='utf-8') as f:
                f.write(f"\n{'='*50}\n")
                f.write(f"Email: {email}\n")
                f.write(f"Password: {password}\n")
                f.write(f"{'='*50}\n")
        
        # 跳过手机号验证流程
        if skip_phone_verification:
            print(f"{Fore.CYAN}🔄 开始跳过手机号验证流程{Style.RESET_ALL}")
            
            # Step 1: 输入邮箱
            print(f"{Fore.CYAN}📧 输入邮箱: {email}{Style.RESET_ALL}")
            email_input = page.ele("@name=email")
            if email_input:
                email_input.input(email)
                time.sleep(get_random_wait_time(config, 'input_wait'))
            else:
                print(f"{Fore.RED}❌ 未找到邮箱输入框{Style.RESET_ALL}")
                return False, page
            
            # Step 2: 点击Continue按钮
            print(f"{Fore.CYAN}🔄 点击Continue按钮{Style.RESET_ALL}")
            continue_button = page.ele("@type=submit")
            if continue_button:
                continue_button.click()
                time.sleep(get_random_wait_time(config, 'submit_wait'))
            else:
                print(f"{Fore.RED}❌ 未找到Continue按钮{Style.RESET_ALL}")
                return False, page
            
            # 检查点击Continue后是否跳转到老验证页面
            current_url = page.url
            
            # 只有检测到老验证页面才进行重试
            max_retry_after_continue = 3
            retry_count_continue = 0
            
            while retry_count_continue < max_retry_after_continue:
                current_url = page.url
                
                if "authenticate.cursor.sh/user_management/initiate_login" in current_url:
                    retry_count_continue += 1
                    print(f"{Fore.YELLOW}⚠️ [Continue后] 检测到老验证页面，重新开始流程 (第{retry_count_continue}次重试){Style.RESET_ALL}")
                    
                    # 重新跳转到注册入口页面
                    page.get("https://authenticator.cursor.sh/")
                    print(f"{Fore.CYAN}🔄 已重新跳转到注册入口{Style.RESET_ALL}")
                    time.sleep(get_random_wait_time(config, 'page_load_wait'))
                    
                    # 重新输入邮箱
                    print(f"{Fore.CYAN}📧 重新输入邮箱: {email}{Style.RESET_ALL}")
                    email_input_retry = page.ele("@name=email")
                    if email_input_retry:
                        email_input_retry.input(email)
                        time.sleep(get_random_wait_time(config, 'input_wait'))
                    else:
                        print(f"{Fore.RED}❌ 重试时未找到邮箱输入框{Style.RESET_ALL}")
                        return False, page
                    
                    # 重新点击Continue
                    continue_button_retry = page.ele("@type=submit")
                    if continue_button_retry:
                        continue_button_retry.click()
                        time.sleep(get_random_wait_time(config, 'submit_wait'))
                    else:
                        print(f"{Fore.RED}❌ 重试时未找到Continue按钮{Style.RESET_ALL}")
                        return False, page
                else:
                    # URL正确，跳出循环（不打印重复日志）
                    break
            
            if retry_count_continue >= max_retry_after_continue:
                print(f"{Fore.RED}❌ 点击Continue后多次重试仍跳转到老验证页面，注册失败{Style.RESET_ALL}")
                return False, None
            
            # Step 3: 等待并点击"用验证码登录"按钮
            print(f"{Fore.CYAN}🔄 等待验证码登录按钮加载...{Style.RESET_ALL}")
            
            # 等待验证码登录按钮出现（最多等待15秒）
            verification_btn = None
            max_wait = 60
            wait_interval = 0.5
            elapsed = 0
            
            while elapsed < max_wait and not verification_btn:
                try:
                    # 方法1: 通过value属性查找
                    verification_btn = page.ele("@value=magic-code", timeout=0.5)
                    if verification_btn:
                        print(f"{Fore.GREEN}✅ 通过value属性找到验证码登录按钮{Style.RESET_ALL}")
                        break
                except:
                    pass
                
                # 方法2: 通过data-method属性查找
                if not verification_btn:
                    try:
                        verification_btn = page.ele("@data-method=email", timeout=0.5)
                        if verification_btn:
                            print(f"{Fore.GREEN}✅ 通过data-method属性找到验证码登录按钮{Style.RESET_ALL}")
                            break
                    except:
                        pass
                
                # 方法3: 通过文本内容查找
                if not verification_btn:
                    try:
                        buttons = page.eles("tag:button", timeout=0.5)
                        for btn in buttons:
                            btn_text = btn.text.lower() if btn.text else ""
                            if "email sign-in code" in btn_text or "sign-in code" in btn_text:
                                verification_btn = btn
                                print(f"{Fore.GREEN}✅ 通过文本找到验证码登录按钮: {btn.text}{Style.RESET_ALL}")
                                break
                    except:
                        pass
                
                if not verification_btn:
                    time.sleep(wait_interval)
                    elapsed += wait_interval
                    if elapsed % 3 == 0:  # 每3秒打印一次
                        print(f"{Fore.CYAN}⏳ 继续等待验证码登录按钮... ({elapsed}秒/{max_wait}秒){Style.RESET_ALL}")
            
            if not verification_btn:
                print(f"{Fore.RED}❌ 等待{max_wait}秒后仍未找到验证码登录按钮{Style.RESET_ALL}")
                return False, page
            
            if verification_btn:
                verification_btn.click()
                print(f"{Fore.GREEN}✅ 点击验证码登录按钮{Style.RESET_ALL}")
                
                # 等待一下让页面有反应时间
                time.sleep(8)
                
                # 检查是否有Cloudflare验证
                print(f"{Fore.CYAN}🔄 检查是否有Cloudflare验证...{Style.RESET_ALL}")
                if handle_turnstile(page, config, translator):
                    print(f"{Fore.GREEN}✅ Cloudflare验证通过{Style.RESET_ALL}")
                else:
                    print(f"{Fore.CYAN}ℹ️ 未检测到Cloudflare验证或已自动通过{Style.RESET_ALL}")
                
                # 等待验证码页面真正加载出来（URL包含magic-code）
                print(f"{Fore.CYAN}⏳ 等待验证码页面加载（检测URL包含magic-code）...{Style.RESET_ALL}")
                magic_code_page_loaded = False
                max_wait_magic = 60
                elapsed_magic = 0
                wait_interval_magic = 0.5
                
                while elapsed_magic < max_wait_magic and not magic_code_page_loaded:
                    try:
                        current_url = page.url
                        if "magic-code" in current_url.lower():
                            magic_code_page_loaded = True
                            print(f"{Fore.GREEN}✅ 验证码页面已加载: {current_url}{Style.RESET_ALL}")
                            break
                    except:
                        pass
                    
                    if not magic_code_page_loaded:
                        time.sleep(wait_interval_magic)
                        elapsed_magic += wait_interval_magic
                        if elapsed_magic % 3 == 0:
                            print(f"{Fore.CYAN}⏳ 等待验证码页面加载... ({elapsed_magic}秒/{max_wait_magic}秒){Style.RESET_ALL}")
                
                if not magic_code_page_loaded:
                    print(f"{Fore.YELLOW}⚠️ 等待{max_wait_magic}秒后未检测到magic-code页面，继续尝试...{Style.RESET_ALL}")
                else:
                    # 页面加载完成后，额外等待一下确保稳定
                    print(f"{Fore.CYAN}⏳ 页面已加载，等待0.5秒确保稳定...{Style.RESET_ALL}")
                    time.sleep(0.5)
                
                # Step 4: 返回上一页
                print(f"{Fore.CYAN}🔄 返回上一页{Style.RESET_ALL}")
                page.back()
                
                # 等待返回上一页后页面加载完成
                print(f"{Fore.CYAN}⏳ 等待返回后页面加载...{Style.RESET_ALL}")
                time.sleep(1.5)
                
                # Step 5: 等待并再次点击验证码登录按钮进入验证码页面
                print(f"{Fore.CYAN}🔄 等待页面加载，准备再次点击验证码登录按钮...{Style.RESET_ALL}")
                verification_btn = None
                max_wait_2 = 60
                wait_interval_2 = 0.5
                elapsed_2 = 0
                
                while elapsed_2 < max_wait_2 and not verification_btn:
                    try:
                        verification_btn = page.ele("@value=magic-code", timeout=0.5)
                        if verification_btn:
                            print(f"{Fore.GREEN}✅ 再次通过value属性找到验证码登录按钮{Style.RESET_ALL}")
                            break
                    except:
                        pass
                    
                    if not verification_btn:
                        try:
                            verification_btn = page.ele("@data-method=email", timeout=0.5)
                            if verification_btn:
                                print(f"{Fore.GREEN}✅ 再次通过data-method属性找到验证码登录按钮{Style.RESET_ALL}")
                                break
                        except:
                            pass
                    
                    if not verification_btn:
                        try:
                            buttons = page.eles("tag:button", timeout=0.5)
                            for btn in buttons:
                                btn_text = btn.text.lower() if btn.text else ""
                                if "email sign-in code" in btn_text or "sign-in code" in btn_text:
                                    verification_btn = btn
                                    print(f"{Fore.GREEN}✅ 再次通过文本找到验证码登录按钮{Style.RESET_ALL}")
                                    break
                        except:
                            pass
                    
                    if not verification_btn:
                        time.sleep(wait_interval_2)
                        elapsed_2 += wait_interval_2
                        if elapsed_2 % 3 == 0:
                            print(f"{Fore.CYAN}⏳ 继续等待验证码登录按钮... ({elapsed_2}秒/{max_wait_2}秒){Style.RESET_ALL}")
                
                if not verification_btn:
                    print(f"{Fore.RED}❌ 返回后等待{max_wait_2}秒仍未找到验证码登录按钮{Style.RESET_ALL}")
                    return False, page
                
                if verification_btn:
                    verification_btn.click()
                    print(f"{Fore.GREEN}✅ 再次点击验证码登录按钮{Style.RESET_ALL}")
                    
                    # 等待一下让页面有反应时间
                    time.sleep(8)
                    
                    # 检查是否有Cloudflare验证
                    print(f"{Fore.CYAN}🔄 检查是否有Cloudflare验证...{Style.RESET_ALL}")
                    if handle_turnstile(page, config, translator):
                        print(f"{Fore.GREEN}✅ Cloudflare验证通过{Style.RESET_ALL}")
                    else:
                        print(f"{Fore.CYAN}ℹ️ 未检测到Cloudflare验证或已自动通过{Style.RESET_ALL}")
                    
                    # 等待验证码输入页面加载（检查验证码输入框是否出现）
                    print(f"{Fore.CYAN}⏳ 等待验证码输入页面加载...{Style.RESET_ALL}")
                    code_input_loaded = False
                    max_wait_code_page = 60
                    elapsed_code_page = 0
                    wait_interval_code = 0.5
                    
                    while elapsed_code_page < max_wait_code_page and not code_input_loaded:
                        try:
                            # 尝试查找验证码输入框（通常是6位数字的输入框）
                            code_inputs = page.eles("tag:input")
                            for inp in code_inputs:
                                input_type = inp.attr("type") or ""
                                input_name = inp.attr("name") or ""
                                # 查找可能的验证码输入框
                                if "code" in input_name.lower() or "otp" in input_name.lower() or input_type == "tel":
                                    code_input_loaded = True
                                    print(f"{Fore.GREEN}✅ 验证码输入页面已加载{Style.RESET_ALL}")
                                    break
                            
                            if not code_input_loaded:
                                # 也可能是URL变化了
                                current_url = page.url
                                if "code" in current_url.lower() or "verify" in current_url.lower():
                                    code_input_loaded = True
                                    print(f"{Fore.GREEN}✅ 已跳转到验证码页面: {current_url}{Style.RESET_ALL}")
                                    break
                        except:
                            pass
                        
                        if not code_input_loaded:
                            time.sleep(wait_interval_code)
                            elapsed_code_page += wait_interval_code
                            if elapsed_code_page % 5 == 0:
                                print(f"{Fore.CYAN}⏳ 等待验证码页面加载... ({elapsed_code_page}秒/{max_wait_code_page}秒){Style.RESET_ALL}")
                    
                    if not code_input_loaded:
                        print(f"{Fore.YELLOW}⚠️ 未检测到验证码输入页面，继续尝试处理...{Style.RESET_ALL}")
                    
                    # 额外等待一下确保页面稳定
                    time.sleep(1)
                    
                    # Step 6: 获取验证码并输入
                    print(f"{Fore.CYAN}📱 开始处理验证码...{Style.RESET_ALL}")
                    if handle_verification_code(page, email_tab, controller, config, translator):
                        success = True
                        return True, page
                    else:
                        print(f"{Fore.RED}❌ 验证码处理失败{Style.RESET_ALL}")
                        return False, page
                else:
                    print(f"{Fore.RED}❌ 再次未找到验证码登录按钮{Style.RESET_ALL}")
                    return False, page
            else:
                print(f"{Fore.RED}❌ 未找到验证码登录按钮{Style.RESET_ALL}")
                return False, page
        
        # 正常注册流程
        # Fill form
        if fill_signup_form(page, first_name, last_name, email, config, translator):
            if translator:
                print(f"\n{Fore.GREEN}✅ {translator.get('register.form_submitted')}{Style.RESET_ALL}")
            
            # Handle first Turnstile verification
            if handle_turnstile(page, config, translator):
                if translator:
                    print(f"\n{Fore.GREEN}✅ {translator.get('register.first_verification_passed')}{Style.RESET_ALL}")
                
                # Fill password
                if fill_password(page, password, config, translator):
                    if translator:
                        print(f"\n{Fore.CYAN}🔄 {translator.get('register.waiting_for_second_verification')}{Style.RESET_ALL}")
                                        
                    # Handle second Turnstile verification
                    if handle_turnstile(page, config, translator):
                        if translator:
                            print(f"\n{Fore.CYAN}🔄 {translator.get('register.waiting_for_verification_code')}{Style.RESET_ALL}")
                        if handle_verification_code(page, email_tab, controller, config, translator):
                            success = True
                            return True, page
                        else:
                            print(f"\n{Fore.RED}❌ {translator.get('register.verification_code_processing_failed') if translator else 'Verification code processing failed'}{Style.RESET_ALL}")
                    else:
                        print(f"\n{Fore.RED}❌ {translator.get('register.second_verification_failed') if translator else 'Second verification failed'}{Style.RESET_ALL}")
                else:
                    print(f"\n{Fore.RED}❌ {translator.get('register.second_verification_failed') if translator else 'Second verification failed'}{Style.RESET_ALL}")
            else:
                print(f"\n{Fore.RED}❌ {translator.get('register.first_verification_failed') if translator else 'First verification failed'}{Style.RESET_ALL}")
        
        return False, None
        
    except Exception as e:
        print(f"发生错误: {e}")
        return False, None
    finally:
        if page and not success:  # Only clean up when failed
            try:
                page.quit()
            except:
                pass
            cleanup_chrome_processes(translator)

if __name__ == "__main__":
    main()  # Run without parameters, use randomly generated information 