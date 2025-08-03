#!/usr/bin/env python3
"""
Setup script to initialize admin password for GitHub Archive Scraper
"""

from auth_manager import AuthManager
import getpass

def setup_admin_password():
    """Setup admin password"""
    print("GitHub Archive Scraper - Admin Setup")
    print("=====================================")
    
    auth_manager = AuthManager()
    
    # Check if admin password already exists
    if auth_manager.auth_config.get('admin_password_hash'):
        print("Admin password already exists!")
        reset = input("Do you want to reset it? (y/N): ").lower()
        if reset != 'y':
            print("Setup cancelled.")
            return
    
    print("\nSetting up admin password...")
    password = input("Enter admin password (or press Enter for default 'admin123'): ")
    
    if not password:
        password = "computergod123"
        print("Using default password: computergod123")
    
    # Set the password
    if auth_manager.set_admin_password(password):
        print("✅ Admin password set successfully!")
        print(f"You can now login to the web interface at http://localhost:8080")
        print(f"Username: admin")
        print(f"Password: {password}")
    else:
        print("❌ Failed to set admin password!")

if __name__ == "__main__":
    setup_admin_password()
