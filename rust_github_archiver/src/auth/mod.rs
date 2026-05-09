// Authentication module
pub mod jwt;
pub mod middleware;
pub mod roles;
pub mod users;

// Re-export main types and functions
pub use jwt::{create_token, create_token_for_user, token_expiration_rfc3339, verify_token};
pub use middleware::{
    admin_auth_middleware, auth_middleware, operator_auth_middleware, optional_auth_middleware,
};
pub use roles::UserRole;
pub use users::{User, UserManager};
