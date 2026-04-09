import argparse
import base64
import hashlib
import json
import secrets
import urllib.parse
from dataclasses import dataclass

# 与 openai_register.py 保持完全一致的 OAuth 常量
AUTH_URL = "https://auth.openai.com/oauth/authorize"
CLIENT_ID = "app_EMoamEEZ73f0CkXaXp7hrann"
DEFAULT_REDIRECT_URI = f"http://localhost:1455/auth/callback"
DEFAULT_SCOPE = "openid email profile offline_access"


def _b64url_no_pad(raw: bytes) -> str:
    return base64.urlsafe_b64encode(raw).decode("ascii").rstrip("=")


def _sha256_b64url_no_pad(s: str) -> str:
    return _b64url_no_pad(hashlib.sha256(s.encode("ascii")).digest())


def _random_state(nbytes: int = 16) -> str:
    return secrets.token_urlsafe(nbytes)


def _pkce_verifier() -> str:
    return secrets.token_urlsafe(64)


@dataclass(frozen=True)
class OAuthStart:
    auth_url: str
    state: str
    code_verifier: str
    redirect_uri: str


def generate_oauth_url(
    *, redirect_uri: str = DEFAULT_REDIRECT_URI, scope: str = DEFAULT_SCOPE
) -> OAuthStart:
    state = _random_state()
    code_verifier = _pkce_verifier()
    code_challenge = _sha256_b64url_no_pad(code_verifier)

    params = {
        "client_id": CLIENT_ID,
        "response_type": "code",
        "redirect_uri": redirect_uri,
        "scope": scope,
        "state": state,
        "code_challenge": code_challenge,
        "code_challenge_method": "S256",
        "prompt": "login",
        "id_token_add_organizations": "true",
        "codex_cli_simplified_flow": "true",
    }
    auth_url = f"{AUTH_URL}?{urllib.parse.urlencode(params)}"
    return OAuthStart(
        auth_url=auth_url,
        state=state,
        code_verifier=code_verifier,
        redirect_uri=redirect_uri,
    )


def run_step1(*, redirect_uri: str, scope: str, output_json: str) -> OAuthStart:
    oauth = generate_oauth_url(redirect_uri=redirect_uri, scope=scope)

    print("=" * 60)
    print("Step 1: Open the URL below in your browser and sign in to OpenAI:")
    print("-" * 60)
    print(oauth.auth_url)
    print("-" * 60)
    print("\nStep 2: After sign-in, the browser will redirect to a localhost error page.")
    print("Copy the full URL that starts with http://localhost:1455/auth/callback...")
    print("\nStep 3: Save the following values for the second script:")
    print(f"STATE: {oauth.state}")
    print(f"VERIFIER: {oauth.code_verifier}")
    print("=" * 60)

    temp_data = {
        "state": oauth.state,
        "code_verifier": oauth.code_verifier,
        "redirect_uri": oauth.redirect_uri,
        "auth_url": oauth.auth_url,
    }
    with open(output_json, "w", encoding="utf-8") as f:
        json.dump(temp_data, f, ensure_ascii=False)
    print(f"\n[Saved intermediate values to {output_json}]")
    return oauth


def main() -> None:
    parser = argparse.ArgumentParser(
        description="OpenAI OAuth Step1: generate auth URL and PKCE parameters"
    )
    parser.add_argument("--redirect-uri", default=DEFAULT_REDIRECT_URI, help="OAuth redirect_uri")
    parser.add_argument("--scope", default=DEFAULT_SCOPE, help="OAuth scope")
    parser.add_argument(
        "--output-json",
        default="temp_oauth_data.json",
        help="Output file to store state/code_verifier/auth_url",
    )
    args = parser.parse_args()

    run_step1(redirect_uri=args.redirect_uri, scope=args.scope, output_json=args.output_json)


if __name__ == "__main__":
    main()
