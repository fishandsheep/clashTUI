use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::util::percent_encode;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub mode: String,
}

#[derive(Clone)]
pub struct MihomoController {
    pub(crate) api_url: String,
    client: Arc<reqwest::Client>,
}

impl MihomoController {
    /// Creates a controller pointing at `api_url` (e.g. `"http://127.0.0.1:9090"`).
    /// Optionally authenticates with `secret` via the `Authorization` header.
    pub fn new(api_url: &str, secret: Option<&str>) -> Self {
        let mut builder = reqwest::Client::builder().no_proxy();

        if let Some(token) = secret {
            let mut headers = reqwest::header::HeaderMap::new();
            if let Ok(val) =
                reqwest::header::HeaderValue::from_str(&format!("Bearer {}", token))
            {
                headers.insert(reqwest::header::AUTHORIZATION, val);
            }
            builder = builder.default_headers(headers);
        }

        let client = builder
            .build()
            .expect("Failed to create HTTP client");

        Self {
            api_url: api_url.to_string(),
            client: Arc::new(client),
        }
    }

    pub async fn get_proxies(&self) -> Result<HashMap<String, serde_json::Value>, String> {
        let url = format!("{}/proxies", self.api_url);
        let resp = self.client.get(&url).send().await;

        match resp {
            Ok(r) => {
                if r.status().is_success() {
                    let body = r.text().await.map_err(|e| format!("Read error: {}", e))?;
                    let data: serde_json::Value = serde_json::from_str(&body)
                        .map_err(|e| format!("JSON parse error: {}", e))?;
                    if let Some(proxies) = data.get("proxies").and_then(|v| v.as_object()) {
                        Ok(proxies
                            .iter()
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect())
                    } else {
                        Err("Invalid response format".to_string())
                    }
                } else {
                    Err(format!("API error: {}", r.status()))
                }
            }
            Err(e) => Err(format!("Request failed: {}", e)),
        }
    }

    pub async fn select_proxy(&self, group: &str, proxy: &str) -> Result<(), String> {
        let url = format!("{}/proxies/{}", self.api_url, percent_encode(group));
        let body = serde_json::json!({ "name": proxy }).to_string();
        let resp = self
            .client
            .put(&url)
            .header("content-type", "application/json")
            .body(body)
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => Ok(()),
            Ok(r) => Err(format!("API error: {}", r.status())),
            Err(e) => Err(format!("Request failed: {}", e)),
        }
    }

    pub async fn switch_mode(&self, mode: &str) -> Result<(), String> {
        let url = format!("{}/configs", self.api_url);
        let body = serde_json::json!({ "mode": mode }).to_string();
        let resp = self
            .client
            .patch(&url)
            .header("content-type", "application/json")
            .body(body)
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => Ok(()),
            Ok(r) => Err(format!("API error: {}", r.status())),
            Err(e) => Err(format!("Request failed: {}", e)),
        }
    }

    pub async fn get_config(&self) -> Result<Config, String> {
        let url = format!("{}/configs", self.api_url);
        let resp = self.client.get(&url).send().await;

        match resp {
            Ok(r) if r.status().is_success() => {
                let body = r.text().await.map_err(|e| format!("Read error: {}", e))?;
                serde_json::from_str(&body).map_err(|e| format!("JSON parse error: {}", e))
            }
            Ok(r) => Err(format!("API error: {}", r.status())),
            Err(e) => Err(format!("Request failed: {}", e)),
        }
    }


}
