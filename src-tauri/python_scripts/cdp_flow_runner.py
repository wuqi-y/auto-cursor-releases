import argparse
import json
import os
import re
import random
import sys
import time
import tempfile
from typing import Any, Dict, List, Optional, Tuple
from urllib.parse import urlparse
import requests

try:
    from colorama import Fore, Style, init  # type: ignore
except Exception:  # pragma: no cover
    class _NoColor:
        BLACK = RED = GREEN = YELLOW = BLUE = MAGENTA = CYAN = WHITE = ""

    class _NoStyle:
        RESET_ALL = ""

    Fore = _NoColor()  # type: ignore
    Style = _NoStyle()  # type: ignore

    def init() -> None:  # type: ignore
        return None

init(strip=False, convert=False, wrap=False)

DEFAULT_MAIL_WORKER_BASE = "https://apimail.xxx.xyz"
DEFAULT_MAIL_ADMIN_AUTH = "AUTOCURSOT_WUQI_2002"
DEFAULT_MAIL_FORWARD_ADDRESS = "cursor@xxx.xyz"
# 若站点启用了私有站点密码，填写后会自动带上 x-custom-auth
MAIL_CUSTOM_AUTH: Optional[str] = None
# 清空收件箱的 inbox id（按你提供的接口截图写死）
MAIL_FORWARD_INBOX_ID = 108

MAIL_API_BASE = (
    os.environ.get("TEST_MAIL_API_BASE")
    or os.environ.get("MAIL_API_BASE")
    or DEFAULT_MAIL_WORKER_BASE
).strip().rstrip("/")
MAIL_ADMIN_TOKEN = (
    os.environ.get("TEST_MAIL_ADMIN_TOKEN")
    or os.environ.get("MAIL_ADMIN_AUTH")
    or DEFAULT_MAIL_ADMIN_AUTH
).strip()
MAIL_FORWARD_ADDRESS = (
    os.environ.get("MAIL_FORWARD_ADDRESS")
    or DEFAULT_MAIL_FORWARD_ADDRESS
).strip()
OAI_CODE_INPUT_TOKEN = "__OAI_CODE__"
OAI_CODE_INPUT_TOKEN_AUTO = "__AUTO__"
RANDOM_NAME_INPUT_TOKEN = "__RANDOM_EN_NAME__"
BIRTHDAY_INPUT_TOKEN = "__BIRTHDAY_2002_03_12__"


def _read_verification_code_from_file(
    timeout_s: float = 180.0,
    poll_interval_s: float = 1.0,
    request_reemit_interval_s: float = 8.0,
) -> str:
    code_file = os.environ.get("CURSOR_VERIFICATION_CODE_FILE", "").strip()
    if not code_file:
        _safe_print("request_verification_code")
        return ""

    _safe_print("request_verification_code")
    end_at = time.time() + max(0.0, timeout_s)
    next_reemit_at = time.time() + max(2.0, request_reemit_interval_s)
    while time.time() < end_at:
        if os.path.exists(code_file):
            try:
                with open(code_file, "r", encoding="utf-8") as f:
                    code = (f.read() or "").strip()
            except Exception:
                code = ""
            if re.fullmatch(r"\d{6}", code):
                try:
                    os.remove(code_file)
                except Exception:
                    pass
                return code
        # 低频重发请求，避免消息桥偶发丢失导致第二次验证码流程“无响应”。
        now = time.time()
        if now >= next_reemit_at:
            _safe_print("request_verification_code")
            next_reemit_at = now + max(2.0, request_reemit_interval_s)
        time.sleep(poll_interval_s)

    _safe_print("manual_input_required")
    return ""


def _mail_poll_debug() -> bool:
    # 默认关闭详细轮询日志，避免常规流程刷屏；需要排障时可手动开启。
    return os.environ.get("CDP_FLOW_MAIL_DEBUG", "0").strip().lower() in ("1", "true", "yes", "y")


def _mail_relax_email_match() -> bool:
    return os.environ.get("CDP_FLOW_MAIL_RELAX_EMAIL", "1").strip().lower() in ("1", "true", "yes", "y")


def _register_email_in_content(content_lower: str, register_email: str) -> bool:
    if not register_email:
        return True
    reg = register_email.strip().lower()
    if reg in content_lower:
        return True
    if _mail_relax_email_match():
        local = reg.split("@", 1)[0]
        if len(local) >= 3 and local in content_lower:
            return True
    return False


def _snip(s: str, n: int = 160) -> str:
    s = re.sub(r"\s+", " ", (s or "").strip())
    return s if len(s) <= n else s[: n - 3] + "..."


def _mail_api_url(path: str) -> str:
    if not MAIL_API_BASE:
        return ""
    return f"{MAIL_API_BASE}{path if path.startswith('/') else ('/' + path)}"



def _mail_admin_headers_get() -> Dict[str, str]:
    h: Dict[str, str] = {
        "Accept": "application/json",
        "x-admin-auth": MAIL_ADMIN_TOKEN,
    }
    if MAIL_CUSTOM_AUTH:
        h["x-custom-auth"] = MAIL_CUSTOM_AUTH
    return h


def _normalize_mail_list(data: Any) -> List[Dict[str, Any]]:
    if isinstance(data, list):
        return [x for x in data if isinstance(x, dict)]
    if isinstance(data, dict):
        for key in ("results", "mails", "data", "items"):
            value = data.get(key)
            if isinstance(value, list):
                return [x for x in value if isinstance(x, dict)]
    return []


def _mail_msg_dedupe_key(msg: Dict[str, Any]) -> str:
    for key in ("id", "_id", "message_id", "uid"):
        value = msg.get(key)
        if value:
            return str(value)
    raw = str(msg.get("raw") or "")
    subject = str(msg.get("subject") or "")
    sender = str(msg.get("from") or msg.get("sender") or msg.get("source") or "")
    return f"{sender}|{subject}|{hash(raw)}"


def _extract_otp_from_openai_mail(content: str) -> str:
    if not content:
        return ""
    candidates = re.findall(r"(?<!\d)(\d{6})(?!\d)", content)
    for code in candidates:
        return code
    return ""


def _otp_reject_six_digit_codes_from_email(email: str) -> set[str]:
    """Collect obvious non-OTP 6-digit numbers from an email-like string."""
    bad: set[str] = set()
    e = (email or "").strip().lower()
    if not e:
        return bad
    bad.update(re.findall(r"(?<!\d)(\d{6})(?!\d)", e))
    if "@" in e:
        domain = e.rsplit("@", 1)[1]
        first = domain.split(".")[0] if domain else ""
        if first.isdigit() and len(first) == 6:
            bad.add(first)
    return bad


def _otp_reject_six_digit_codes_from_context(register_email: str) -> set[str]:
    bad: set[str] = set()
    bad.update(_otp_reject_six_digit_codes_from_email(register_email))
    bad.update(_otp_reject_six_digit_codes_from_email(MAIL_FORWARD_ADDRESS))
    try:
        host = (urlparse(MAIL_API_BASE).hostname or "").lower()
    except Exception:
        host = ""
    if host:
        bad.update(re.findall(r"(?<!\d)(\d{6})(?!\d)", host))
    return bad



def _extract_otp_from_openai_mail_with_skip(content: str, skip_otp: set[str]) -> str:
    """优先匹配正文里的「Your ChatGPT code is 615067」句式（可夹 HTML），否则再扫任意 6 位数字。"""
    # 先做一次通用降噪，避免把颜色值/区号样式/域名片段误识别为验证码。
    sanitized = content or ""
    # 1) #414141 这类颜色值
    sanitized = re.sub(r"#([0-9a-fA-F]{6})\b", " ", sanitized)
    # 2) +123456 这类带前缀号段
    sanitized = re.sub(r"\+\d{6}\b", " ", sanitized)
    # 3) 801304.xyz 这类域名前缀
    sanitized = re.sub(r"\b\d{6}\.[a-zA-Z][a-zA-Z0-9.-]*", " ", sanitized)

    # Highest priority: explicit "123456\r" style token from raw MIME/text bodies.
    m_cr = re.search(r"(?<!\d)(\d{6})\r(?!\d)", sanitized)
    if m_cr:
        code = m_cr.group(1)
        if code not in skip_otp:
            return code

    m = re.search(
        r"(?is)Your\s+ChatGPT\s+code\s+is\D*?(\d{6})(?!\d)",
        sanitized,
    )
    if m:
        code = m.group(1)
        if code not in skip_otp:
            return code
    for m2 in re.finditer(r"(?<!\d)(\d{6})(?!\d)", sanitized):
        code = m2.group(1)
        if code in skip_otp:
            continue
        return code
    return ""


def clear_forward_inbox(proxies: Any = None) -> bool:
    """每次获取验证码前清空 cursor 收件箱，避免读到旧验证码。"""
    try:
        resp = requests.delete(
            _mail_api_url(f"/admin/clear_inbox/{MAIL_FORWARD_INBOX_ID}"),
            headers=_mail_admin_headers_get(),
            proxies=proxies,
            timeout=15,
        )
        if resp.status_code in (200, 204):
            return True
        print(f"Mail Worker 清空收件箱: {resp}")
        return False
    except Exception as e:
        print(f"[Error] 清空收件箱失败: {e}")
        return False


def get_oai_code(register_email: str, proxies: Any = None) -> str:
    """
    Poll worker /admin/mails and extract OpenAI six-digit OTP.
    Clears forward inbox before each polling attempt sequence.
    """
    debug = _mail_poll_debug()
    seen_keys: set[str] = set()
    register_email = (register_email or "").strip()
    skip_otp = _otp_reject_six_digit_codes_from_context(register_email)

    clear_forward_inbox(proxies)
    _safe_print(
        f"{Fore.CYAN}[*] 等待验证码 register_email={register_email or '(unknown)'} "
        f"forward={MAIL_FORWARD_ADDRESS or '(unset)'} api_base={MAIL_API_BASE or '(unset)'}{Style.RESET_ALL}"
    )
    if skip_otp and debug:
        _safe_print(f"{Fore.YELLOW}[mail] 将忽略上下文中的 6 位数: {sorted(skip_otp)}{Style.RESET_ALL}")

    api = _mail_api_url("/admin/mails")
    if not api:
        _safe_print(" 未配置 TEST_MAIL_API_BASE，跳过自动拉取验证码")
        return ""

    for attempt in range(40):
        try:
            try:
                # Keep this call shape aligned with gpt-80.py
                resp = requests.get(
                    api,
                    params={"limit": "20", "offset": "0", "address": MAIL_FORWARD_ADDRESS},
                    headers=_mail_admin_headers_get(),
                    proxies=proxies,
                    impersonate="chrome",
                    timeout=15,
                )
            except TypeError:
                # Fallback when runtime requests implementation doesn't support impersonate
                resp = requests.get(
                    api,
                    params={"limit": "20", "offset": "0", "address": MAIL_FORWARD_ADDRESS},
                    headers=_mail_admin_headers_get(),
                    proxies=proxies,
                    timeout=15,
                )

            email_list: List[Dict[str, Any]] = []
            if resp.status_code == 200:
                try:
                    data = resp.json()
                except Exception as je:
                    if debug:
                        _safe_print(
                            f"{Fore.RED}[mail] {attempt + 1}/40 JSON 解析失败: {je} body_snip={_snip(resp.text)}{Style.RESET_ALL}"
                        )
                    time.sleep(3)
                    continue
                email_list = _normalize_mail_list(data)
                if debug and not email_list and isinstance(data, dict):
                    _safe_print(
                        f"{Fore.YELLOW}[mail] {attempt + 1}/40 邮件列表为空；响应顶层键: {list(data.keys())}{Style.RESET_ALL}"
                    )
            elif debug:
                _safe_print(
                    f"{Fore.YELLOW}[mail] {attempt + 1}/40 HTTP {resp.status_code} snip={_snip(getattr(resp, 'text', '') or '')}{Style.RESET_ALL}"
                )

            if debug:
                _safe_print(
                    f"{Fore.CYAN}[mail] {attempt + 1}/40 HTTP={resp.status_code} 邮件数={len(email_list)}{Style.RESET_ALL}"
                )

            if resp.status_code != 200:
                time.sleep(3)
                continue

            for mi, msg in enumerate(email_list):
                if not isinstance(msg, dict):
                    continue
                key = _mail_msg_dedupe_key(msg)
                if key in seen_keys:
                    if debug:
                        _safe_print(f"  {Fore.MAGENTA}msg#{mi} dedupe_skip key={_snip(key, 80)}{Style.RESET_ALL}")
                    continue
                seen_keys.add(key)

                sender = str(
                    msg.get("from")
                    or msg.get("sender")
                    or msg.get("from_address")
                    or msg.get("source")
                    or ""
                ).lower()
                subject = str(msg.get("subject", ""))
                body = str(msg.get("body") or msg.get("text") or msg.get("content") or "")
                html = str(msg.get("html") or msg.get("html_body") or "")
                raw = str(msg.get("raw") or "")
                content = "\n".join([sender, subject, body, html, raw])
                content_lower = content.lower()

                head = f"msg#{mi} sender={_snip(sender, 48)} subj={_snip(subject, 60)}"

                if "openai" not in sender and "openai" not in content_lower:
                    if debug:
                        _safe_print(f"  {Fore.YELLOW}{head} -> skip (无 openai 发件/正文){Style.RESET_ALL}")
                    continue
                if not _register_email_in_content(content_lower, register_email):
                    if debug:
                        _safe_print(
                            f"  {Fore.YELLOW}{head} -> skip (收件人匹配失败; register_email={register_email!r} "
                            f"RELAX={_mail_relax_email_match()}){Style.RESET_ALL}"
                        )
                    continue

                code = _extract_otp_from_openai_mail_with_skip(content, skip_otp)
                if code:
                    _safe_print(f"{Fore.GREEN}[mail] {head} -> 抓到验证码: {code}{Style.RESET_ALL}")
                    return code
                raw_cands = re.findall(r"(?<!\d)(\d{6})(?!\d)", content)
                if debug:
                    _safe_print(
                        f"  {Fore.RED}{head} -> openai+邮箱命中但未解析出码; 候选6位数={raw_cands[:12]} skip_set={skip_otp}{Style.RESET_ALL}"
                    )
        except Exception as e:
            if debug:
                _safe_print(f"{Fore.RED}[mail] {attempt + 1}/40 异常: {e}{Style.RESET_ALL}")
        time.sleep(3)

    _safe_print(f"{Fore.RED}[mail] 超时，未收到验证码（共 40 轮）{Style.RESET_ALL}")
    return ""


def _random_en_name() -> str:
    # Simple English-like "First Last" without external deps.
    first = "".join(random.choices("abcdefghijklmnopqrstuvwxyz", k=random.randint(4, 8))).capitalize()
    last = "".join(random.choices("abcdefghijklmnopqrstuvwxyz", k=random.randint(5, 10))).capitalize()
    return f"{first} {last}"


def _looks_like_name_field(selector: str) -> bool:
    s = (selector or "").lower()
    return (
        "@name=name" in s
        or "name=name" in s
        or "name='name'" in s
        or "name=\"name\"" in s
    )


def _looks_like_birthday_field(selector: str) -> bool:
    s = (selector or "").lower()
    return (
        "@name=birthday" in s
        or "name=birthday" in s
        or "name='birthday'" in s
        or "name=\"birthday\"" in s
    )


def _looks_like_otp_field(selector: str) -> bool:
    s = (selector or "").lower()
    return (
        "@name=otp" in s
        or "name=otp" in s
        or "name='otp'" in s
        or "name=\"otp\"" in s
        or "@name=code" in s
        or "name=code" in s
        or "verificationcode" in s
    )


def _safe_print(*args: Any, **kwargs: Any) -> None:
    try:
        print(*args, **kwargs)
        sys.stdout.flush()
    except Exception:
        pass


def _emit_event(payload: Dict[str, Any]) -> None:
    _safe_print(json.dumps(payload, ensure_ascii=False))


def _wait_for_continue(
    *,
    reason: str,
    prompt: str,
    continue_file: Optional[str] = None,
    cancel_file: Optional[str] = None,
    poll_interval_s: float = 0.5,
) -> bool:
    """
    Block until user signals "continue" via temp file.

    - Writes JSON event for the frontend to react.
    - Continue file content: "continue" (case-insensitive) or any non-empty string.
    - Cancel file content: "cancel" (case-insensitive) or existence of cancel file.
    """
    temp_dir = tempfile.gettempdir()
    continue_file = continue_file or os.environ.get(
        "CDP_FLOW_CONTINUE_FILE", os.path.join(temp_dir, "cdp_flow_continue.txt")
    )
    cancel_file = cancel_file or os.environ.get(
        "CDP_FLOW_CANCEL_FILE", os.path.join(temp_dir, "cdp_flow_cancel.txt")
    )

    for fp in (continue_file, cancel_file):
        if fp and os.path.exists(fp):
            try:
                os.remove(fp)
            except Exception:
                pass

    _emit_event(
        {
            "action": "wait_for_user",
            "reason": reason,
            "message": prompt,
            "continue_file": continue_file,
            "cancel_file": cancel_file,
            "status": "waiting",
        }
    )
    _safe_print(
        f"{Fore.CYAN}⏳ {prompt}{Style.RESET_ALL}\n"
        f"{Fore.CYAN}ℹ️ continue_file: {continue_file}{Style.RESET_ALL}\n"
        f"{Fore.CYAN}ℹ️ cancel_file:   {cancel_file}{Style.RESET_ALL}"
    )

    while True:
        if cancel_file and os.path.exists(cancel_file):
            return False

        if continue_file and os.path.exists(continue_file):
            try:
                with open(continue_file, "r", encoding="utf-8") as f:
                    content = (f.read() or "").strip().lower()
            except Exception:
                content = "continue"

            try:
                os.remove(continue_file)
            except Exception:
                pass

            if content == "cancel":
                return False
            return True

        time.sleep(poll_interval_s)


def _export_cookies(page: Any) -> List[Dict[str, Any]]:
    try:
        cookies = page.cookies()
        if isinstance(cookies, list):
            return cookies
        return []
    except Exception:
        return []


def _export_storage(page: Any) -> Dict[str, Any]:
    js = r"""
(() => {
  const dump = (s) => {
    try {
      const out = {};
      for (let i = 0; i < s.length; i++) {
        const k = s.key(i);
        out[k] = s.getItem(k);
      }
      return out;
    } catch (e) {
      return {};
    }
  };
  return { localStorage: dump(window.localStorage), sessionStorage: dump(window.sessionStorage) };
})()
"""
    try:
        data = page.run_js(js)
        if isinstance(data, dict):
            return data
        return {"localStorage": {}, "sessionStorage": {}}
    except Exception:
        return {"localStorage": {}, "sessionStorage": {}}


def _try_click(page: Any, selector: str, timeout_s: float = 10.0) -> bool:
    """
    Click a DOM element using DrissionPage selector syntax.
    Examples:
      - "css:button[data-testid='signup']"
      - "xpath://button[contains(., 'Sign up')]"
      - "@type=submit"
    """
    try:
        ele = page.ele(selector, timeout=timeout_s)
        if not ele:
            return False
        ele.click()
        return True
    except Exception:
        return False


def _try_input(page: Any, selector: str, value: str, timeout_s: float = 10.0) -> bool:
    try:
        ele = page.ele(selector, timeout=timeout_s)
        if not ele:
            return False
        try:
            # 聚焦输入框，降低验证码框输入丢失概率
            ele.click()
        except Exception:
            pass
        try:
            ele.clear()
        except Exception:
            pass
        ele.input(value)
        return True
    except Exception:
        return False


def _wait_page_loaded(page: Any, timeout_s: float = 20.0) -> bool:
    """
    Best-effort wait for page load complete.
    Tries DrissionPage wait APIs if available; falls back to polling document.readyState.
    """
    # 1) DrissionPage style wait if present
    try:
        w = getattr(page, "wait", None)
        if w is not None:
            for method_name in ("load_complete", "load", "doc_loaded"):
                fn = getattr(w, method_name, None)
                if callable(fn):
                    try:
                        fn(timeout=timeout_s)
                        return True
                    except Exception:
                        pass
    except Exception:
        pass

    # 2) Fallback: poll document.readyState
    end_at = time.time() + max(0.0, timeout_s)
    while time.time() < end_at:
        try:
            state = page.run_js("return document.readyState")  # type: ignore[attr-defined]
            if str(state).lower() == "complete":
                return True
        except Exception:
            pass
        time.sleep(0.25)
    return False


def _is_oauth_callback_url(url: str, redirect_uri: str) -> bool:
    u = (url or "").strip()
    r = (redirect_uri or "").strip()
    if not u or not r:
        return False
    return u.startswith(r)


def _run_oauth_step2_with_callback(final_url: str) -> None:
    callback_url = (final_url or "").strip()
    if not callback_url:
        return
    try:
        from openai_oauth_step2 import run_step2  # type: ignore
    except Exception as e:
        _safe_print(f"{Fore.YELLOW}⚠️ 无法导入 openai_oauth_step2.run_step2: {e}{Style.RESET_ALL}")
        return

    try:
        temp_path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "temp_oauth_data.json")
        out_file = run_step2(callback_url, temp_path)
        _safe_print(f"{Fore.GREEN}✅ OAuth Step2 成功，已生成: {out_file}{Style.RESET_ALL}")
    except Exception as e:
        _safe_print(f"{Fore.YELLOW}⚠️ OAuth Step2 换取 Token 失败: {e}{Style.RESET_ALL}")


def _run_post_oauth_step1_in_same_browser(page: Any, register_email_hint: str) -> None:
    """在当前浏览器中直接打开 OpenAI OAuth 授权 URL，并复用同一邮箱+自动验证码。"""
    try:
        from openai_oauth_step1 import generate_oauth_url  # type: ignore
    except Exception as e:
        _safe_print(f"{Fore.YELLOW}⚠️ 无法导入 openai_oauth_step1.generate_oauth_url: {e}{Style.RESET_ALL}")
        return

    try:
        oauth = generate_oauth_url()
    except Exception as e:
        _safe_print(f"{Fore.YELLOW}⚠️ 生成 OAuth URL 失败: {e}{Style.RESET_ALL}")
        return

    # 等待上一流程页面加载完毕后再额外等待 2 秒，再打开 OAuth 页面
    _wait_page_loaded(page, timeout_s=30.0)
    time.sleep(2.0)

    auth_url = oauth.auth_url
    redirect_uri = getattr(oauth, "redirect_uri", "http://localhost:1455/auth/callback")

    # 同步写入 temp_oauth_data.json，保持与 openai_oauth_step1 一致的输出文件
    try:
        temp_path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "temp_oauth_data.json")
        temp_data = {
            "state": oauth.state,
            "code_verifier": oauth.code_verifier,
            "redirect_uri": oauth.redirect_uri,
            "auth_url": oauth.auth_url,
        }
        with open(temp_path, "w", encoding="utf-8") as f:
            json.dump(temp_data, f, ensure_ascii=False)
        _safe_print(f"{Fore.CYAN}[OAuth Step1] Saved intermediate values to {temp_path}{Style.RESET_ALL}")
    except Exception as e:
        _safe_print(f"{Fore.YELLOW}⚠️ 写入 temp_oauth_data.json 失败: {e}{Style.RESET_ALL}")
    _safe_print(f"{Fore.CYAN}🔐 OAuth Step1 auth_url: {auth_url}{Style.RESET_ALL}")

    # 优先尝试新开一个 tab，失败则在当前 tab 跳转
    new_page = page
    try:
        if hasattr(page, "new_tab"):
            candidate = page.new_tab(auth_url)  # type: ignore[attr-defined]
            # DrissionPage new_tab 可能返回 None 或新 Tab 对象，优先用返回值
            if candidate is not None:
                new_page = candidate
        else:
            page.get(auth_url)
    except Exception:
        try:
            page.get(auth_url)
        except Exception as e:
            _safe_print(f"{Fore.YELLOW}⚠️ 打开 OAuth URL 失败: {e}{Style.RESET_ALL}")
            return

    try:
        # 若 new_tab 返回了新的页面对象，则使用它
        if new_page is None:
            new_page = page
    except Exception:
        new_page = page

    # 1) 输入邮箱（沿用注册流程中使用的邮箱）
    email = (register_email_hint or "").strip()
    if not email:
        _safe_print(f"{Fore.YELLOW}⚠️ OAuth Step1: 未检测到注册邮箱，跳过自动填写{Style.RESET_ALL}")
        return

    _safe_print(f"{Fore.CYAN}📧 OAuth Step1 使用邮箱: {email}{Style.RESET_ALL}")

    email_selectors = [
        "css:input#email",
        "@name=email",
        "css:input[type='email']",
        "css:input[name*='email']",
    ]
    email_ok = False
    for sel in email_selectors:
        if _try_input(new_page, sel, email, timeout_s=20.0):
            _safe_print(
                f"{Fore.CYAN}⌨️ OAuth Step1 input email into {sel!r}: OK{Style.RESET_ALL}"
            )
            email_ok = True
            break
        else:
            _safe_print(
                f"{Fore.YELLOW}⚠️ OAuth Step1: 邮箱输入失败 selector={sel!r}{Style.RESET_ALL}"
            )

    if not email_ok:
        _safe_print(
            f"{Fore.RED}❌ OAuth Step1: 所有候选邮箱输入框 selector 均未命中，放弃后续自动步骤{Style.RESET_ALL}"
        )
        return

    # 2) 先点 submit（Continue / 下一步），再点 intent（页面顺序如此）
    if not _try_click(new_page, "css:button[type='submit']", timeout_s=20.0):
        _safe_print(f"{Fore.YELLOW}⚠️ OAuth Step1: 首次未找到提交按钮 css:button[type='submit']{Style.RESET_ALL}")
        return
    _safe_print(f"{Fore.CYAN}🖱️ OAuth Step1 首次 click 'css:button[type=\"submit\"]': OK{Style.RESET_ALL}")
    _wait_page_loaded(new_page, timeout_s=30.0)
    time.sleep(2.0)

    # 3) 再点 intent（排除 type=submit）
    intent_selectors = [
        "xpath://button[@name='intent' and not(@type='submit')]",
        "xpath://*[@name='intent' and not(@type='submit')]",
    ]
    intent_ok = False
    for sel in intent_selectors:
        if _try_click(new_page, sel, timeout_s=20.0):
            _safe_print(f"{Fore.CYAN}🖱️ OAuth Step1 click intent {sel!r}: OK{Style.RESET_ALL}")
            intent_ok = True
            break
        _safe_print(f"{Fore.YELLOW}⚠️ OAuth Step1 intent MISS selector={sel!r}{Style.RESET_ALL}")

    if not intent_ok:
        _safe_print(
            f"{Fore.YELLOW}⚠️ OAuth Step1: 未点到 name=intent（页面可能无此步），继续尝试后续 submit{Style.RESET_ALL}"
        )

    _wait_page_loaded(new_page, timeout_s=30.0)
    time.sleep(2.0)

    # 4) 再点 submit（发送验证码或进入 OTP 页）
    if not _try_click(new_page, "css:button[type='submit']", timeout_s=20.0):
        _safe_print(f"{Fore.YELLOW}⚠️ OAuth Step1: 第二次未找到提交按钮 css:button[type='submit']{Style.RESET_ALL}")
        return
    _safe_print(f"{Fore.CYAN}🖱️ OAuth Step1 第二次 click 'css:button[type=\"submit\"]': OK{Style.RESET_ALL}")
    _wait_page_loaded(new_page, timeout_s=30.0)
    time.sleep(2.0)

    # 5) 自动获取验证码并填入
    _safe_print(f"{Fore.CYAN}📨 OAuth Step1: 开始自动获取验证码{Style.RESET_ALL}")
    code = _read_verification_code_from_file() if os.environ.get("CURSOR_VERIFICATION_CODE_FILE") else get_oai_code(email)
    if not code:
        _safe_print(f"{Fore.RED}❌ OAuth Step1: 自动获取验证码失败{Style.RESET_ALL}")
        return

    # 优先 @name=otp，失败回退 @name=code（与主流程保持一致）
    ok = _try_input(new_page, "@name=otp", code, timeout_s=20.0)
    _safe_print(f"{Fore.CYAN}⌨️ OAuth Step1 input '@name=otp': {('OK' if ok else 'MISS')}{Style.RESET_ALL}")
    if (not ok) and _looks_like_otp_field("@name=otp"):
        fallback_sel = "@name=code"
        _safe_print(f"{Fore.YELLOW}↩️ OAuth Step1: otp 输入失败，回退 {fallback_sel!r}{Style.RESET_ALL}")
        ok2 = _try_input(new_page, fallback_sel, code, timeout_s=20.0)
        _safe_print(f"{Fore.CYAN}⌨️ OAuth Step1 input {fallback_sel!r}: {('OK' if ok2 else 'MISS')}{Style.RESET_ALL}")
        ok = ok2

    if not ok:
        _safe_print(f"{Fore.RED}❌ OAuth Step1: 验证码输入失败{Style.RESET_ALL}")
        return

    # 6) 连续两次点击提交：每次点击后等待页面加载完毕，再额外等待 2 秒
    for i in range(2):
        submit_ok = _try_click(new_page, "css:button[type='submit']", timeout_s=20.0)
        _safe_print(
            f"{Fore.CYAN}🖱️ OAuth Step1 第 {i+1} 次 click 'css:button[type=\"submit\"]': "
            f"{('OK' if submit_ok else 'MISS')}{Style.RESET_ALL}"
        )
        # 关键：提交后通常会跳转/渲染，先等 load complete，再等 2 秒更稳
        _wait_page_loaded(new_page, timeout_s=30.0)
        time.sleep(2.0)

    # 7) 等待页面加载并打印最终 URL
    time.sleep(3.0)
    final_url = ""
    try:
        final_url = new_page.url  # type: ignore[attr-defined]
    except Exception:
        pass

    if final_url:
        _safe_print(f"{Fore.GREEN}✅ OAuth Step1 最终页面 URL: {final_url}{Style.RESET_ALL}")
    else:
        _safe_print(f"{Fore.YELLOW}⚠️ OAuth Step1: 无法获取最终页面 URL{Style.RESET_ALL}")

    # 若未跳到 localhost callback，则继续点击 submit 重试并更新最终 URL
    if final_url and not _is_oauth_callback_url(final_url, redirect_uri):
        max_retry = 3
        for r in range(max_retry):
            submit_ok = _try_click(new_page, "css:button[type='submit']", timeout_s=20.0)
            _safe_print(
                f"{Fore.YELLOW}↻ OAuth Step1 回调未出现，重试 submit {r + 1}/{max_retry}: "
                f"{('OK' if submit_ok else 'MISS')}{Style.RESET_ALL}"
            )
            _wait_page_loaded(new_page, timeout_s=30.0)
            time.sleep(2.0)
            try:
                final_url = new_page.url  # type: ignore[attr-defined]
            except Exception:
                final_url = final_url
            if final_url:
                _safe_print(f"{Fore.CYAN}🔄 OAuth Step1 当前 URL: {final_url}{Style.RESET_ALL}")
            if final_url and _is_oauth_callback_url(final_url, redirect_uri):
                _safe_print(f"{Fore.GREEN}✅ OAuth Step1 已到达回调 URL: {final_url}{Style.RESET_ALL}")
                break

    if final_url and _is_oauth_callback_url(final_url, redirect_uri):
        _run_oauth_step2_with_callback(final_url)


def run_flow(
    *,
    url: str,
    use_incognito: bool = True,
    custom_config_json: Optional[str] = None,
    steps: Optional[List[Tuple[str, str]]] = None,
    clicks: Optional[List[str]] = None,
    inputs: Optional[List[Tuple[str, str]]] = None,
    wait_after_open_s: float = 2.0,
    wait_after_action_s: float = 1.0,
    element_timeout_s: float = 10.0,
    code_submit_selector: str = "css:button[type='submit']",
    post_oauth_step1_py: Optional[str] = None,
    pause_reason: str = "manual_step",
    pause_prompt: str = "请在浏览器中完成必要的手动步骤，然后写入 continue_file 继续。",
) -> int:
    # Lazy import so `--help` works without DrissionPage installed.
    try:
        from new_signup import setup_driver  # Reuse existing DrissionPage driver setup
    except Exception as e:
        _emit_event(
            {
                "action": "error",
                "status": "missing_dependency",
                "message": "无法导入浏览器驱动（new_signup/DrissionPage）。请确保已安装依赖或使用项目自带的打包运行环境。",
                "detail": str(e),
            }
        )
        return 3

    custom_config: Optional[Dict[str, Any]] = None
    if custom_config_json:
        try:
            custom_config = json.loads(custom_config_json)
        except Exception:
            custom_config = None

    config, page = setup_driver(translator=None, use_incognito=use_incognito, custom_config=custom_config)

    try:
        page.get("about:blank")
        time.sleep(0.5)

        _safe_print(f"{Fore.CYAN}🌐 打开: {url}{Style.RESET_ALL}")
        page.get(url)
        time.sleep(max(0.0, wait_after_open_s))

        ordered_steps: List[Tuple[str, str]] = []
        register_email_hint = ""
        if steps:
            ordered_steps.extend(steps)
        else:
            # Backward-compat: clicks first, then inputs
            for sel in clicks or []:
                ordered_steps.append(("click", sel))
            for sel, val in inputs or []:
                ordered_steps.append(("input", f"{sel}={val}"))

        for kind, payload in ordered_steps:
            if kind == "click":
                sel = payload
                ok = _try_click(page, sel, timeout_s=element_timeout_s)
                _safe_print(
                    f"{Fore.CYAN}🖱️ click {sel!r}: {('OK' if ok else 'MISS')}{Style.RESET_ALL}"
                )
                time.sleep(max(0.0, wait_after_action_s))
                continue

            if kind == "input":
                if "=" not in payload:
                    _safe_print(f"{Fore.YELLOW}⚠️ input 参数格式错误: {payload!r}{Style.RESET_ALL}")
                    continue
                sel, val = payload.rsplit("=", 1)
                sel = sel.strip()
                val = val.strip()

                if "@" in val and "." in val:
                    register_email_hint = val

                if val == RANDOM_NAME_INPUT_TOKEN and _looks_like_name_field(sel):
                    val = _random_en_name()

                if val == BIRTHDAY_INPUT_TOKEN and _looks_like_birthday_field(sel):
                    val = "2002-03-12"

                should_auto_fetch_code = (
                    val == OAI_CODE_INPUT_TOKEN
                    or (val == OAI_CODE_INPUT_TOKEN_AUTO and _looks_like_otp_field(sel))
                )
                if should_auto_fetch_code:
                    _safe_print(f"{Fore.CYAN}📨 触发自动获取验证码: {sel!r}{Style.RESET_ALL}")
                    code = _read_verification_code_from_file() if os.environ.get("CURSOR_VERIFICATION_CODE_FILE") else get_oai_code(register_email_hint)
                    if not code:
                        _safe_print(
                            f"{Fore.RED}❌ 自动获取验证码失败，请检查测试邮箱接口配置{Style.RESET_ALL}"
                        )
                        return 4
                    val = code
                ok = _try_input(page, sel, val, timeout_s=element_timeout_s)
                _safe_print(
                    f"{Fore.CYAN}⌨️ input {sel!r}: {('OK' if ok else 'MISS')}{Style.RESET_ALL}"
                )
                if (not ok) and should_auto_fetch_code and sel.strip().lower() in ("@name=otp", "name=otp"):
                    fallback_sel = "@name=code"
                    _safe_print(f"{Fore.YELLOW}↩️ otp 输入失败，回退尝试 {fallback_sel!r}{Style.RESET_ALL}")
                    ok2 = _try_input(page, fallback_sel, val, timeout_s=element_timeout_s)
                    _safe_print(
                        f"{Fore.CYAN}⌨️ input {fallback_sel!r}: {('OK' if ok2 else 'MISS')}{Style.RESET_ALL}"
                    )
                    ok = ok2
                if ok and val.isdigit() and len(val) == 6 and should_auto_fetch_code:
                    submit_ok = _try_click(page, code_submit_selector, timeout_s=element_timeout_s)
                    _safe_print(
                        f"{Fore.CYAN}🖱️ click {code_submit_selector!r}: {('OK' if submit_ok else 'MISS')}{Style.RESET_ALL}"
                    )
                time.sleep(max(0.0, wait_after_action_s))
                continue

            _safe_print(f"{Fore.YELLOW}⚠️ 未知 step 类型: {kind!r}{Style.RESET_ALL}")

        if post_oauth_step1_py:
            _run_post_oauth_step1_in_same_browser(page, register_email_hint)

        # 默认不阻塞等待 continue_file：输出提示后即可结束流程，避免长时间挂起。
        # 如需保留旧行为，可设置 CDP_FLOW_REQUIRE_CONTINUE=true。
        require_continue = os.environ.get("CDP_FLOW_REQUIRE_CONTINUE", "0").strip().lower() in (
            "1",
            "true",
            "yes",
            "y",
        )
        if require_continue:
            cont = _wait_for_continue(reason=pause_reason, prompt=pause_prompt)
            if not cont:
                _emit_event({"action": "cancelled", "status": "cancelled"})
                return 2
        else:
            _safe_print(f"{Fore.CYAN}ℹ️ 跳过 continue_file 等待（CDP_FLOW_REQUIRE_CONTINUE=0）{Style.RESET_ALL}")

        cookies = _export_cookies(page)
        storage = _export_storage(page)
        current_url = ""
        title = ""
        try:
            current_url = page.url
        except Exception:
            pass
        try:
            title = page.title
        except Exception:
            pass

        _emit_event(
            {
                "action": "flow_result",
                "status": "completed",
                "url": current_url,
                "title": title,
                "cookies": cookies,
                "storage": storage,
            }
        )
        return 0
    finally:
        keep_open = os.environ.get("CDP_FLOW_KEEP_BROWSER_OPEN", "1").lower() in ("1", "true", "yes", "y")
        if not keep_open:
            try:
                page.quit()
            except Exception:
                pass


def _parse_inputs(items: List[str]) -> List[Tuple[str, str]]:
    out: List[Tuple[str, str]] = []
    for raw in items:
        if "=" not in raw:
            continue
        sel, val = raw.rsplit("=", 1)
        sel = sel.strip()
        if not sel:
            continue
        out.append((sel, val))
    return out


def _parse_ordered_steps_from_argv(argv: List[str]) -> List[Tuple[str, str]]:
    """
    Preserve the interleaving order of --click/--input as written by the user.
    Supports both:
      --click "<selector>"
      --click="<selector>"
      --input "<selector>=<value>"
      --input="<selector>=<value>"
    """
    steps: List[Tuple[str, str]] = []
    i = 0
    while i < len(argv):
        tok = argv[i]
        if tok == "--click":
            if i + 1 < len(argv):
                steps.append(("click", argv[i + 1]))
                i += 2
                continue
        if tok.startswith("--click="):
            steps.append(("click", tok.split("=", 1)[1]))
            i += 1
            continue

        if tok == "--input":
            if i + 1 < len(argv):
                steps.append(("input", argv[i + 1]))
                i += 2
                continue
        if tok.startswith("--input="):
            steps.append(("input", tok.split("=", 1)[1]))
            i += 1
            continue

        i += 1
    return steps


def main() -> None:
    parser = argparse.ArgumentParser(description="CDP Flow Runner (DrissionPage) - 可配置流程骨架")
    parser.add_argument("--url", required=True, help="目标 URL（仅用于你有权限自动化的站点）")
    parser.add_argument("--incognito", default="true", help="是否无痕：true/false")
    parser.add_argument("--custom-config-json", default=None, help="透传给 setup_driver 的 JSON 配置（字符串）")
    parser.add_argument("--click", action="append", default=[], help="要点击的 selector（可多次传入）")
    parser.add_argument(
        "--input",
        action="append",
        default=[],
        help="输入项：'<selector>=<value>'（可多次传入）",
    )
    parser.add_argument("--wait-after-open", type=float, default=2.0)
    parser.add_argument("--wait-after-action", type=float, default=1.0)
    parser.add_argument("--element-timeout", type=float, default=10.0, help="每个 click/input 等待元素出现的最长秒数")
    parser.add_argument(
        "--post-oauth-step1-py",
        default=None,
        help="可选：流程步骤结束后自动执行 OAuth Step1 脚本（如 openai_oauth_step1.py）",
    )
    parser.add_argument(
        "--code-submit-selector",
        default="css:button[type='submit']",
        help=f"当 --input 的值为 {OAI_CODE_INPUT_TOKEN} 或 OTP 输入框值为 {OAI_CODE_INPUT_TOKEN_AUTO} 时，输入验证码后自动点击该按钮",
    )
    args = parser.parse_args()

    use_incognito = str(args.incognito).strip().lower() in ("1", "true", "yes", "y")
    ordered_steps = _parse_ordered_steps_from_argv(sys.argv[1:])
    inputs = _parse_inputs(args.input or [])

    exit_code = run_flow(
        url=args.url,
        use_incognito=use_incognito,
        custom_config_json=args.custom_config_json,
        steps=ordered_steps if ordered_steps else None,
        clicks=args.click or [],
        inputs=inputs,
        wait_after_open_s=args.wait_after_open,
        wait_after_action_s=args.wait_after_action,
        element_timeout_s=args.element_timeout,
        code_submit_selector=args.code_submit_selector,
        post_oauth_step1_py=args.post_oauth_step1_py,
    )
    raise SystemExit(exit_code)


if __name__ == "__main__":
    main()
