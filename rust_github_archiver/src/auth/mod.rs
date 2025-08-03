// Authentication module
pub mod jwt;
pub mod users;
pub mod middleware;

// Re-export main types and functions
pub use jwt::{create_token};
pub use users::{User, UserManager};
pub use middleware::{auth_middleware, optional_auth_middleware};
