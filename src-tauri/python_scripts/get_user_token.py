import requests
import json
import time
from colorama import Fore, Style
import os
from config import get_config

# Define emoji constants
EMOJI = {
    'START': '🚀',
    'OAUTH': '🔑',
    'SUCCESS': '✅',
    'ERROR': '❌',
    'WAIT': '⏳',
    'INFO': 'ℹ️',
    'WARNING': '⚠️'
}

def refresh_token(token, translator=None):
    """Refresh the token using the Chinese server API
    
    Args:
        token (str): The full WorkosCursorSessionToken cookie value
        translator: Optional translator object
        
    Returns:
        str: The refreshed access token or original token if refresh fails
    """
    try:
        config = get_config(translator)
        # Get refresh_server URL from config or use default
        refresh_server = config.get('Token', 'refresh_server', fallback='https://token.cursorpro.com.cn')
        
        # Ensure the token is URL encoded properly
        if '%3A%3A' not in token and '::' in token:
            # Replace :: with URL encoded version if needed
            token = token.replace('::', '%3A%3A')
            
        # Make the request to the refresh server
        url = f"{refresh_server}/reftoken?token={token}"
        
        print(f"{Fore.CYAN}{EMOJI['INFO']} {translator.get('token.refreshing') if translator else 'Refreshing token...'}{Style.RESET_ALL}")
        
        response = requests.get(url, timeout=30)
        
        if response.status_code == 200:
            try:
                data = response.json()
                
                if data.get('code') == 0 and data.get('msg') == "获取成功":
                    access_token = data.get('data', {}).get('accessToken')
                    days_left = data.get('data', {}).get('days_left', 0)
                    expire_time = data.get('data', {}).get('expire_time', 'Unknown')
                    
                    if access_token:
                        print(f"{Fore.GREEN}{EMOJI['SUCCESS']} {translator.get('token.refresh_success', days=days_left, expire=expire_time) if translator else f'Token refreshed successfully! Valid for {days_left} days (expires: {expire_time})'}{Style.RESET_ALL}")
                        return access_token
                    else:
                        print(f"{Fore.YELLOW}{EMOJI['WARNING']} {translator.get('token.no_access_token') if translator else 'No access token in response'}{Style.RESET_ALL}")
                else:
                    error_msg = data.get('msg', 'Unknown error')
                    print(f"{Fore.RED}{EMOJI['ERROR']} {translator.get('token.refresh_failed', error=error_msg) if translator else f'Token refresh failed: {error_msg}'}{Style.RESET_ALL}")
            except json.JSONDecodeError:
                print(f"{Fore.RED}{EMOJI['ERROR']} {translator.get('token.invalid_response') if translator else 'Invalid JSON response from refresh server'}{Style.RESET_ALL}")
        else:
            print(f"{Fore.RED}{EMOJI['ERROR']} {translator.get('token.server_error', status=response.status_code) if translator else f'Refresh server error: HTTP {response.status_code}'}{Style.RESET_ALL}")
    
    except requests.exceptions.Timeout:
        print(f"{Fore.RED}{EMOJI['ERROR']} {translator.get('token.request_timeout') if translator else 'Request to refresh server timed out'}{Style.RESET_ALL}")
    except requests.exceptions.ConnectionError:
        print(f"{Fore.RED}{EMOJI['ERROR']} {translator.get('token.connection_error') if translator else 'Connection error to refresh server'}{Style.RESET_ALL}")
    except Exception as e:
        print(f"{Fore.RED}{EMOJI['ERROR']} {translator.get('token.unexpected_error', error=str(e)) if translator else f'Unexpected error during token refresh: {str(e)}'}{Style.RESET_ALL}")
    
    # Return original token if refresh fails
    return token.split('%3A%3A')[-1] if '%3A%3A' in token else token.split('::')[-1] if '::' in token else token

def get_token_from_cookie(cookie_value, translator=None):
    """Extract and process token from cookie value
    
    Args:
        cookie_value (str): The WorkosCursorSessionToken cookie value
        translator: Optional translator object
        
    Returns:
        str: The processed token
    """
    try:
        # Try to refresh the token with the API first
        refreshed_token = refresh_token(cookie_value, translator)
        
        # If refresh succeeded and returned a different token, use it
        if refreshed_token and refreshed_token != cookie_value:
            return refreshed_token
        
        # If refresh failed or returned same token, use traditional extraction method
        if '%3A%3A' in cookie_value:
            return cookie_value.split('%3A%3A')[-1]
        elif '::' in cookie_value:
            return cookie_value.split('::')[-1]
        else:
            return cookie_value
            
    except Exception as e:
        print(f"{Fore.RED}{EMOJI['ERROR']} {translator.get('token.extraction_error', error=str(e)) if translator else f'Error extracting token: {str(e)}'}{Style.RESET_ALL}")
        # Fall back to original behavior
        if '%3A%3A' in cookie_value:
            return cookie_value.split('%3A%3A')[-1]
        elif '::' in cookie_value:
            return cookie_value.split('::')[-1]
        else:
            return cookie_value 