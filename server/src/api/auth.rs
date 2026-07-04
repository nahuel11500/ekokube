//! HTTP Basic Auth guard for the UI + API.
//!
//! Enabled when a username is configured. Browsers handle the `WWW-Authenticate`
//! challenge natively (login prompt), so the static UI and the JSON API are
//! protected uniformly with no client-side code. `/api/health` is left open so
//! the kubelet liveness probe doesn't need credentials.

use axum::body::Body;
use axum::extract::State;
use axum::http::{header, Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use base64::Engine;

#[derive(Clone)]
pub struct BasicAuth {
    username: String,
    password: String,
}

impl BasicAuth {
    /// Returns `Some` when auth is configured (username non-empty).
    pub fn from_config(username: &str, password: &str) -> Option<Self> {
        if username.is_empty() {
            return None;
        }
        Some(Self {
            username: username.to_string(),
            password: password.to_string(),
        })
    }

    fn matches(&self, header_value: &str) -> bool {
        let Some(encoded) = header_value.strip_prefix("Basic ") else {
            return false;
        };
        let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(encoded.trim()) else {
            return false;
        };
        let Ok(creds) = String::from_utf8(decoded) else {
            return false;
        };
        let Some((user, pass)) = creds.split_once(':') else {
            return false;
        };
        // Constant-time-ish compare is overkill for a shared static credential;
        // a plain compare is fine here.
        user == self.username && pass == self.password
    }
}

/// Applied unconditionally; a `None` state means auth is disabled (pass-through),
/// so the layer type is the same whether or not credentials are configured.
pub async fn guard(
    State(auth): State<Option<BasicAuth>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let Some(auth) = auth else {
        return next.run(request).await;
    };
    // Operational endpoints stay unauthenticated: liveness + readiness probes
    // and the Prometheus /metrics scrape. They expose no tenant/cost data.
    let path = request.uri().path();
    if path == "/api/health" || path == "/api/health/ready" || path == "/metrics" {
        return next.run(request).await;
    }
    let authorized = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(|v| auth.matches(v))
        .unwrap_or(false);
    if authorized {
        return next.run(request).await;
    }
    (
        StatusCode::UNAUTHORIZED,
        [(header::WWW_AUTHENTICATE, "Basic realm=\"ekokube\"")],
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header_for(user: &str, pass: &str) -> String {
        let enc = base64::engine::general_purpose::STANDARD.encode(format!("{user}:{pass}"));
        format!("Basic {enc}")
    }

    #[test]
    fn disabled_when_no_username() {
        assert!(BasicAuth::from_config("", "x").is_none());
        assert!(BasicAuth::from_config("admin", "x").is_some());
    }

    #[test]
    fn matches_correct_credentials() {
        let auth = BasicAuth::from_config("admin", "s3cret").unwrap();
        assert!(auth.matches(&header_for("admin", "s3cret")));
        assert!(!auth.matches(&header_for("admin", "wrong")));
        assert!(!auth.matches(&header_for("root", "s3cret")));
        assert!(!auth.matches("Bearer token"));
        assert!(!auth.matches("Basic not-base64!!"));
    }
}
