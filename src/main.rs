use axum::{
    body::Body,
    extract::State,
    http::{Request, Response, StatusCode},
    Router,
};
use http_body_util::BodyExt;
use reqwest::Client;
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    let client = Client::new();

    let app = Router::new().fallback(proxy_handler).with_state(client);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn proxy_handler(
    State(client): State<Client>,
    req: Request<Body>,
) -> Result<Response<Body>, (StatusCode, String)> {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let headers = req.headers().clone();

    // Look you can do deterministic routing
    let target_host = if method == hyper::Method::POST && uri.path() == "/query" {
        "http://0.0.0.0:4000"
    } else {
        "http://0.0.0.0:3000"
    };

    let path_and_query = uri.path_and_query().map(|pq| pq.as_str()).unwrap_or("");
    let forwarded_uri = format!("{}{}", target_host, path_and_query);

    // Convert the body to bytes
    let body_bytes = req.collect().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Body error: {}", e),
        )
    })?;

    // Start creating the proxied request
    let request_builder = client
        .request(method, &forwarded_uri)
        .headers(headers)
        .body(body_bytes.to_bytes());

    let res = request_builder
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Request error: {}", e)))?;

    // Build the response
    let mut response_builder = Response::builder().status(res.status());

    // Copy headers from the response
    for (key, value) in res.headers().iter() {
        response_builder = response_builder.header(key, value.clone());
    }

    // Read the response body
    let res_body_bytes = res.bytes().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Response error: {}", e),
        )
    })?;

    // Set the response body
    let body = Body::from(res_body_bytes);

    // Build the final response
    let response = response_builder.body(body).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Build error: {}", e),
        )
    })?;

    Ok(response)
}
