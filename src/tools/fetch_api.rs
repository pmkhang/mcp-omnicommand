use reqwest::{
    Client, Method,
    header::{HeaderMap, HeaderName, HeaderValue},
};
use serde_json::{Value, json};
use std::time::Duration;

pub fn info() -> Value {
    json!({
        "name": "fetch_api",
        "description": "Make HTTP requests to any URL (curl-like interface).",
        "inputSchema": {
            "type": "object",
            "properties": {
                "url": { "type": "string", "description": "The URL for the request" },
                "method": {
                    "type": "string",
                    "description": "HTTP method (GET, POST, PUT, DELETE, etc.)",
                    "default": "GET"
                },
                "headers": {
                    "type": "object",
                    "description": "HTTP headers as key-value pairs"
                },
                "body": {
                    "type": "string",
                    "description": "Request body for POST/PUT/PATCH requests"
                },
                "timeout": {
                    "type": "number",
                    "description": "Request timeout in milliseconds (default 30000)",
                    "default": 30000
                }
            },
            "required": ["url"]
        }
    })
}

pub async fn run(arguments: &Value) -> Result<Value, String> {
    let url = arguments
        .get("url")
        .and_then(Value::as_str)
        .ok_or("URL is required")?;
    let method_str = arguments
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or("GET")
        .to_uppercase();
    let timeout_ms = arguments
        .get("timeout")
        .and_then(Value::as_u64)
        .unwrap_or(30000);
    let body_str = arguments.get("body").and_then(Value::as_str);

    let method = Method::from_bytes(method_str.as_bytes()).map_err(|_| "Invalid HTTP method")?;

    let client = Client::builder()
        .timeout(Duration::from_millis(timeout_ms))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

    let mut request_builder = client.request(method, url);

    if let Some(headers_obj) = arguments.get("headers").and_then(|v| v.as_object()) {
        let mut headers = HeaderMap::new();
        for (key, value) in headers_obj {
            let name = HeaderName::from_bytes(key.as_bytes())
                .map_err(|_| format!("Invalid header name: {key}"))?;
            let val_str = value
                .as_str()
                .ok_or_else(|| format!("Header value for {key} must be a string"))?;
            let val = HeaderValue::from_str(val_str)
                .map_err(|_| format!("Invalid header value for {key}"))?;
            headers.insert(name, val);
        }
        request_builder = request_builder.headers(headers);
    }

    if let Some(body) = body_str {
        request_builder = request_builder.body(body.to_string());
    }

    let response = request_builder
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;

    let status = response.status().as_u16();
    let text_content = response.text().await.unwrap_or_default();

    // Try to parse as JSON for prettier output if possible
    let body_json = serde_json::from_str::<Value>(&text_content).unwrap_or(json!(text_content));

    let result = json!({
        "status": status,
        "body": body_json
    });

    Ok(
        json!([{ "type": "text", "text": serde_json::to_string_pretty(&result).unwrap_or_default() }]),
    )
}
