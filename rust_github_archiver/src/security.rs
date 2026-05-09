use axum::{
    extract::Request,
    http::{header, HeaderMap, HeaderName, HeaderValue, StatusCode},
    middleware::Next,
    response::Response,
};
use tracing::warn;

/// Security headers configuration
#[derive(Debug, Clone)]
pub struct SecurityConfig {
    /// Enable HSTS (HTTP Strict Transport Security)
    pub hsts_enabled: bool,
    /// HSTS max age in seconds
    pub hsts_max_age: u32,
    /// Include subdomains in HSTS
    pub hsts_include_subdomains: bool,
    /// Enable CSP (Content Security Policy)
    pub csp_enabled: bool,
    /// CSP policy
    pub csp_policy: String,
    /// Allowed CORS origins
    pub cors_origins: Vec<String>,
    /// Enable X-Frame-Options
    pub frame_options_enabled: bool,
    /// X-Frame-Options value
    pub frame_options: String,
    /// Enable X-Content-Type-Options
    pub content_type_options_enabled: bool,
    /// Enable X-XSS-Protection
    pub xss_protection_enabled: bool,
    /// Referrer policy
    pub referrer_policy: String,
    /// Permissions policy
    pub permissions_policy: String,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            hsts_enabled: true,
            hsts_max_age: 31536000, // 1 year
            hsts_include_subdomains: true,
            csp_enabled: true,
            csp_policy: "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: https:; font-src 'self'; connect-src 'self'; frame-ancestors 'none'".to_string(),
            cors_origins: vec!["https://github-archiver.example.com".to_string()],
            frame_options_enabled: true,
            frame_options: "DENY".to_string(),
            content_type_options_enabled: true,
            xss_protection_enabled: true,
            referrer_policy: "strict-origin-when-cross-origin".to_string(),
            permissions_policy: "geolocation=(), microphone=(), camera=()".to_string(),
        }
    }
}

/// Middleware to add security headers to all responses
pub async fn security_headers_middleware(req: Request, next: Next) -> Result<Response, StatusCode> {
    let config = req
        .extensions()
        .get::<SecurityConfig>()
        .cloned()
        .unwrap_or_default();

    let mut response = next.run(req).await;
    let headers = response.headers_mut();

    // HSTS - Force HTTPS
    if config.hsts_enabled {
        let hsts_value = if config.hsts_include_subdomains {
            format!(
                "max-age={}; includeSubDomains; preload",
                config.hsts_max_age
            )
        } else {
            format!("max-age={}", config.hsts_max_age)
        };

        insert_header_if_valid(headers, header::STRICT_TRANSPORT_SECURITY, &hsts_value);
    }

    // CSP - Prevent XSS and injection attacks
    if config.csp_enabled {
        insert_header_if_valid(headers, header::CONTENT_SECURITY_POLICY, &config.csp_policy);
    }

    // X-Frame-Options - Prevent clickjacking
    if config.frame_options_enabled {
        insert_header_if_valid(
            headers,
            HeaderName::from_static("x-frame-options"),
            &config.frame_options,
        );
    }

    // X-Content-Type-Options - Prevent MIME sniffing
    if config.content_type_options_enabled {
        headers.insert(
            "X-Content-Type-Options",
            HeaderValue::from_static("nosniff"),
        );
    }

    // X-XSS-Protection - Legacy XSS protection
    if config.xss_protection_enabled {
        headers.insert(
            "X-XSS-Protection",
            HeaderValue::from_static("1; mode=block"),
        );
    }

    // Referrer-Policy - Control referrer information
    insert_header_if_valid(headers, header::REFERRER_POLICY, &config.referrer_policy);

    // Permissions-Policy - Control browser features
    insert_header_if_valid(
        headers,
        HeaderName::from_static("permissions-policy"),
        &config.permissions_policy,
    );

    // Remove server header (don't expose server info)
    headers.remove(header::SERVER);

    // Remove X-Powered-By header
    headers.remove("X-Powered-By");

    Ok(response)
}

/// CORS middleware configuration
#[derive(Debug, Clone)]
pub struct CorsConfig {
    /// Allowed origins
    pub allowed_origins: Vec<String>,
    /// Allowed methods
    pub allowed_methods: Vec<String>,
    /// Allowed headers
    pub allowed_headers: Vec<String>,
    /// Exposed headers
    pub exposed_headers: Vec<String>,
    /// Allow credentials
    pub allow_credentials: bool,
    /// Max age for preflight cache
    pub max_age: u32,
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            allowed_origins: vec!["https://github-archiver.example.com".to_string()],
            allowed_methods: vec![
                "GET".to_string(),
                "POST".to_string(),
                "PUT".to_string(),
                "DELETE".to_string(),
                "OPTIONS".to_string(),
            ],
            allowed_headers: vec![
                "Authorization".to_string(),
                "Content-Type".to_string(),
                "X-Request-ID".to_string(),
            ],
            exposed_headers: vec![
                "X-RateLimit-Limit".to_string(),
                "X-RateLimit-Remaining".to_string(),
                "X-RateLimit-Reset".to_string(),
            ],
            allow_credentials: true,
            max_age: 3600, // 1 hour
        }
    }
}

/// CORS middleware
pub async fn cors_middleware(req: Request, next: Next) -> Result<Response, StatusCode> {
    let config = req
        .extensions()
        .get::<CorsConfig>()
        .cloned()
        .unwrap_or_default();

    let origin = req
        .headers()
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string(); // Clone origin before moving req

    let method = req.method().clone(); // Clone method before moving req

    // Handle preflight requests
    if method == "OPTIONS" {
        let mut response = Response::new(String::new().into());
        add_cors_headers(&mut response, &config, &origin);
        return Ok(response);
    }

    let mut response = next.run(req).await;
    add_cors_headers(&mut response, &config, &origin);

    Ok(response)
}

fn add_cors_headers(response: &mut Response, config: &CorsConfig, origin: &str) {
    let headers = response.headers_mut();

    // Check if origin is allowed
    let is_allowed = config.allowed_origins.contains(&origin.to_string())
        || config.allowed_origins.contains(&"*".to_string());

    if is_allowed {
        insert_header_if_valid(headers, header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
    }

    if config.allow_credentials {
        headers.insert(
            header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
            HeaderValue::from_static("true"),
        );
    }

    insert_header_if_valid(
        headers,
        header::ACCESS_CONTROL_ALLOW_METHODS,
        &config.allowed_methods.join(", "),
    );

    insert_header_if_valid(
        headers,
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        &config.allowed_headers.join(", "),
    );

    insert_header_if_valid(
        headers,
        header::ACCESS_CONTROL_EXPOSE_HEADERS,
        &config.exposed_headers.join(", "),
    );

    insert_header_if_valid(
        headers,
        header::ACCESS_CONTROL_MAX_AGE,
        &config.max_age.to_string(),
    );
}

fn insert_header_if_valid(headers: &mut HeaderMap, name: HeaderName, value: &str) {
    match HeaderValue::from_str(value) {
        Ok(value) => {
            headers.insert(name, value);
        }
        Err(error) => {
            warn!("Skipping invalid response header {}: {}", name, error);
        }
    }
}

/// Request size limit middleware
pub async fn request_size_limit_middleware(
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    const MAX_BODY_SIZE: u64 = 10 * 1024 * 1024; // 10MB
    const MAX_HEADER_SIZE: usize = 8 * 1024; // 8KB

    // Check header size
    let header_size: usize = req
        .headers()
        .iter()
        .map(|(name, value)| name.as_str().len() + value.len())
        .sum();

    if header_size > MAX_HEADER_SIZE {
        return Err(StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE);
    }

    // Check content-length header
    if let Some(content_length) = req.headers().get(header::CONTENT_LENGTH) {
        if let Ok(length_str) = content_length.to_str() {
            if let Ok(length) = length_str.parse::<u64>() {
                if length > MAX_BODY_SIZE {
                    return Err(StatusCode::PAYLOAD_TOO_LARGE);
                }
            }
        }
    }

    Ok(next.run(req).await)
}

/// Request timeout middleware
pub async fn request_timeout_middleware(req: Request, next: Next) -> Result<Response, StatusCode> {
    const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

    match tokio::time::timeout(REQUEST_TIMEOUT, next.run(req)).await {
        Ok(response) => Ok(response),
        Err(_) => Err(StatusCode::REQUEST_TIMEOUT),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_config_default() {
        let config = SecurityConfig::default();

        assert!(config.hsts_enabled);
        assert_eq!(config.hsts_max_age, 31536000);
        assert!(config.csp_enabled);
        assert!(config.csp_policy.contains("script-src 'self'"));
        assert!(!config
            .csp_policy
            .contains("script-src 'self' 'unsafe-inline'"));
        assert!(config.frame_options_enabled);
    }

    #[test]
    fn test_cors_config_default() {
        let config = CorsConfig::default();

        assert!(config.allow_credentials);
        assert_eq!(config.max_age, 3600);
        assert!(config.allowed_methods.contains(&"GET".to_string()));
        assert!(config.allowed_methods.contains(&"POST".to_string()));
    }
}
