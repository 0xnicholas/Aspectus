//! OpenAPI documentation endpoints.

use axum::{
    http::{HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
};

/// The OpenAPI 3.0 spec, embedded at compile time so the binary is self-contained.
const OPENAPI_YAML: &str = include_str!("../../../../docs/openapi.yaml");

/// Serve the raw `openapi.yaml` spec.
pub async fn openapi_spec() -> Response {
    (
        StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static("application/yaml"),
        )],
        OPENAPI_YAML,
    )
        .into_response()
}

/// Serve a Swagger UI page that loads `/openapi.yaml`.
pub async fn swagger_ui() -> Html<&'static str> {
    Html(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>Aspectus API Documentation</title>
  <link rel="stylesheet" href="https://unpkg.com/swagger-ui-dist@5/swagger-ui.css">
</head>
<body>
  <div id="swagger-ui"></div>
  <script src="https://unpkg.com/swagger-ui-dist@5/swagger-ui-bundle.js"></script>
  <script>
    window.onload = () => {
      SwaggerUIBundle({
        url: '/openapi.yaml',
        dom_id: '#swagger-ui',
      });
    };
  </script>
</body>
</html>"#,
    )
}
