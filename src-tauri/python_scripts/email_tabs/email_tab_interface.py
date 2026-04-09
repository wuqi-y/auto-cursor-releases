from abc import ABC, abstractmethod

class EmailTabInterface(ABC):
    """Interface for email tab implementations"""
    
    @abstractmethod
    def refresh_inbox(self) -> None:
        """Refresh the email inbox"""
        pass
    
    @abstractmethod
    def check_for_cursor_email(self) -> bool:
        """Check if there is a new email from Cursor
        
        Returns:
            bool: True if new email found, False otherwise
        """
        pass
    
    @abstractmethod
    def get_verification_code(self) -> str:
        """Get the verification code from the email
        
        Returns:
            str: The verification code if available, empty string otherwise
        """
        pass
