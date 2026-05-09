use axum::{extract::Request, http::StatusCode, middleware::Next, response::Response};

pub async fn cors_middleware(request: Request, next: Next) -> Result<Response, StatusCode> {
    crate::security::cors_middleware(request, next).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::CorsConfig;
    use axum::{
        body::Body,
        http::{header, Method, Request},
        middleware,
        routing::get,
        Extension, Router,
    };
    use tower::util::ServiceExt;

    fn test_config() -> CorsConfig {
        CorsConfig {
            allowed_origins: vec!["http://localhost:3000".to_string()],
            ..CorsConfig::default()
        }
    }

    #[tokio::test]
    async fn cors_wrapper_adds_origin_headers_for_allowed_origins() {
        let app = Router::new()
            .route("/", get(|| async { "ok" }))
            .layer(middleware::from_fn(cors_middleware))
            .layer(Extension(test_config()));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header(header::ORIGIN, "http://localhost:3000")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");

        assert_eq!(
            response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN],
            "http://localhost:3000"
        );
    }

    #[tokio::test]
    async fn cors_wrapper_handles_preflight_requests() {
        let app = Router::new()
            .route("/", get(|| async { "ok" }))
            .layer(middleware::from_fn(cors_middleware))
            .layer(Extension(test_config()));

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::OPTIONS)
                    .uri("/")
                    .header(header::ORIGIN, "http://localhost:3000")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");

        assert!(response
            .headers()
            .contains_key(header::ACCESS_CONTROL_ALLOW_METHODS));
        assert!(response
            .headers()
            .contains_key(header::ACCESS_CONTROL_ALLOW_HEADERS));
    }
}
