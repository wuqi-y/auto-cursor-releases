import sqlite3
import os
import sys
from colorama import Fore, Style, init
from config import get_config

# Initialize colorama
init()

# Define emoji and color constants
EMOJI = {
    'DB': '🗄️',
    'UPDATE': '🔄',
    'SUCCESS': '✅',
    'ERROR': '❌',
    'WARN': '⚠️',
    'INFO': 'ℹ️',
    'FILE': '📄',
    'KEY': '🔐'
}

class CursorAuth:
    def __init__(self, translator=None):
        self.translator = translator
        
        # Get configuration
        config = get_config(translator)
        if not config:
            print(f"{Fore.RED}{EMOJI['ERROR']} {self.translator.get('auth.config_error') if self.translator else 'Failed to load configuration'}{Style.RESET_ALL}")
            sys.exit(1)
            
        # Get path based on operating system
        try:
            if sys.platform == "win32":  # Windows
                if not config.has_section('WindowsPaths'):
                    raise ValueError("Windows paths not configured")
                self.db_path = config.get('WindowsPaths', 'sqlite_path')
                
            elif sys.platform == 'linux':  # Linux
                if not config.has_section('LinuxPaths'):
                    raise ValueError("Linux paths not configured")
                self.db_path = config.get('LinuxPaths', 'sqlite_path')
                
            elif sys.platform == 'darwin':  # macOS
                if not config.has_section('MacPaths'):
                    raise ValueError("macOS paths not configured")
                self.db_path = config.get('MacPaths', 'sqlite_path')
                
            else:
                print(f"{Fore.RED}{EMOJI['ERROR']} {self.translator.get('auth.unsupported_platform') if self.translator else 'Unsupported platform'}{Style.RESET_ALL}")
                sys.exit(1)
                
            # Verify if the path exists
            if not os.path.exists(os.path.dirname(self.db_path)):
                raise FileNotFoundError(f"Database directory not found: {os.path.dirname(self.db_path)}")
                
        except Exception as e:
            print(f"{Fore.RED}{EMOJI['ERROR']} {self.translator.get('auth.path_error', error=str(e)) if self.translator else f'Error getting database path: {str(e)}'}{Style.RESET_ALL}")
            sys.exit(1)

        # Check if the database file exists
        if not os.path.exists(self.db_path):
            print(f"{Fore.RED}{EMOJI['ERROR']} {self.translator.get('auth.db_not_found', path=self.db_path) if self.translator else f'Database not found: {self.db_path}'}{Style.RESET_ALL}")
            return

        # Check file permissions
        if not os.access(self.db_path, os.R_OK | os.W_OK):
            print(f"{Fore.RED}{EMOJI['ERROR']} {self.translator.get('auth.db_permission_error') if self.translator else 'Database permission error'}{Style.RESET_ALL}")
            return

        try:
            self.conn = sqlite3.connect(self.db_path)
            print(f"{Fore.GREEN}{EMOJI['SUCCESS']} {self.translator.get('auth.connected_to_database') if self.translator else 'Connected to database'}{Style.RESET_ALL}")
        except sqlite3.Error as e:
            print(f"{Fore.RED}{EMOJI['ERROR']} {self.translator.get('auth.db_connection_error', error=str(e)) if self.translator else f'Database connection error: {str(e)}'}{Style.RESET_ALL}")
            return

    def update_auth(self, email=None, access_token=None, refresh_token=None, auth_type="Auth_0"):
        conn = None
        try:
            # Ensure the directory exists and set the correct permissions
            db_dir = os.path.dirname(self.db_path)
            if not os.path.exists(db_dir):
                os.makedirs(db_dir, mode=0o755, exist_ok=True)
            
            # If the database file does not exist, create a new one
            if not os.path.exists(self.db_path):
                conn = sqlite3.connect(self.db_path)
                cursor = conn.cursor()
                cursor.execute('''
                    CREATE TABLE IF NOT EXISTS ItemTable (
                        key TEXT PRIMARY KEY,
                        value TEXT
                    )
                ''')
                conn.commit()
                if sys.platform != "win32":
                    os.chmod(self.db_path, 0o644)
                conn.close()

            # Reconnect to the database
            conn = sqlite3.connect(self.db_path)
            print(f"{EMOJI['INFO']} {Fore.GREEN} {self.translator.get('auth.connected_to_database') if self.translator else 'Connected to database'}{Style.RESET_ALL}")
            cursor = conn.cursor()
            
            # Add timeout and other optimization settings
            conn.execute("PRAGMA busy_timeout = 5000")
            conn.execute("PRAGMA journal_mode = WAL")
            conn.execute("PRAGMA synchronous = NORMAL")
            
            # Set the key-value pairs to update
            updates = []

            updates.append(("cursorAuth/cachedSignUpType", auth_type))

            if email is not None:
                updates.append(("cursorAuth/cachedEmail", email))
            if access_token is not None:
                updates.append(("cursorAuth/accessToken", access_token))
            if refresh_token is not None:
                updates.append(("cursorAuth/refreshToken", refresh_token))
                

            # Use transactions to ensure data integrity
            cursor.execute("BEGIN TRANSACTION")
            try:
                for key, value in updates:
                    # Check if the key exists
                    cursor.execute("SELECT COUNT(*) FROM ItemTable WHERE key = ?", (key,))
                    if cursor.fetchone()[0] == 0:
                        cursor.execute("""
                            INSERT INTO ItemTable (key, value) 
                            VALUES (?, ?)
                        """, (key, value))
                    else:
                        cursor.execute("""
                            UPDATE ItemTable SET value = ?
                            WHERE key = ?
                        """, (value, key))
                    print(f"{EMOJI['INFO']} {Fore.CYAN} {self.translator.get('auth.updating_pair') if self.translator else 'Updating key-value pair:'} {key.split('/')[-1]}...{Style.RESET_ALL}")
                
                cursor.execute("COMMIT")
                print(f"{EMOJI['SUCCESS']} {Fore.GREEN}{self.translator.get('auth.database_updated_successfully') if self.translator else 'Database updated successfully'}{Style.RESET_ALL}")
                return True
                
            except Exception as e:
                cursor.execute("ROLLBACK")
                raise e

        except sqlite3.Error as e:
            print(f"\n{EMOJI['ERROR']} {Fore.RED} {self.translator.get('auth.database_error', error=str(e)) if self.translator else f'Database error: {str(e)}'}{Style.RESET_ALL}")
            return False
        except Exception as e:
            print(f"\n{EMOJI['ERROR']} {Fore.RED} {self.translator.get('auth.an_error_occurred', error=str(e)) if self.translator else f'An error occurred: {str(e)}'}{Style.RESET_ALL}")
            return False
        finally:
            if conn:
                conn.close()
                print(f"{EMOJI['DB']} {Fore.CYAN} {self.translator.get('auth.database_connection_closed') if self.translator else 'Database connection closed'}{Style.RESET_ALL}")