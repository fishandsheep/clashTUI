use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

const MIHOMO_API_ADDR: &str = "127.0.0.1:9090";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub mode: String,
}

#[derive(Clone)]
pub struct MihomoController {
    #[allow(dead_code)]
    mihomo_path: String,
    #[allow(dead_code)]
    config_path: String,
    pub(crate) api_url: String,
    client: Arc<reqwest::Client>,
}

impl MihomoController {
    pub fn new(mihomo_path: &str, config_path: &str) -> Self {
        // 创建禁用代理的客户端（避免 API 请求被 mihomo 代理）
        let client = reqwest::Client::builder()
            .no_proxy()
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))
            .unwrap();

        Self {
            mihomo_path: expand_tilde(mihomo_path),
            config_path: expand_tilde(config_path),
            api_url: format!("http://{}", MIHOMO_API_ADDR),
            client: Arc::new(client),
        }
    }

    #[allow(dead_code)]
    pub fn start(&self) -> Result<(), String> {
        use std::process::Command;
        let output = Command::new(&self.mihomo_path)
            .arg("-d")
            .arg(std::path::Path::new(&self.config_path)
                .parent()
                .map(|p| p.to_str().unwrap_or("."))
                .unwrap_or("."))
            .arg("-f")
            .arg(&self.config_path)
            .spawn();

        match output {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Failed to start mihomo: {}", e)),
        }
    }

    #[allow(dead_code)]
    pub fn stop(&self) -> Result<(), String> {
        use std::process::Command;
        let output = Command::new("pkill")
            .arg("-f")
            .arg("mihomo")
            .output();

        match output {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Failed to stop mihomo: {}", e)),
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
                        let mut result = HashMap::new();
                        for (k, v) in proxies {
                            result.insert(k.clone(), v.clone());
                        }
                        Ok(result)
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
        let url = format!("{}/proxies/{}", self.api_url, group);
        let body = serde_json::json!({ "name": proxy }).to_string();
        let resp = self.client.put(&url).header("content-type", "application/json").body(body).send().await;

        match resp {
            Ok(r) => {
                if r.status().is_success() {
                    Ok(())
                } else {
                    Err(format!("API error: {}", r.status()))
                }
            }
            Err(e) => Err(format!("Request failed: {}", e)),
        }
    }

    pub async fn switch_mode(&self, mode: &str) -> Result<(), String> {
        let url = format!("{}/configs", self.api_url);
        let body = serde_json::json!({ "mode": mode }).to_string();
        let resp = self.client.patch(&url).header("content-type", "application/json").body(body).send().await;

        match resp {
            Ok(r) => {
                if r.status().is_success() {
                    Ok(())
                } else {
                    Err(format!("API error: {}", r.status()))
                }
            }
            Err(e) => Err(format!("Request failed: {}", e)),
        }
    }

    pub async fn get_config(&self) -> Result<Config, String> {
        let url = format!("{}/configs", self.api_url);
        let resp = self.client.get(&url).send().await;

        match resp {
            Ok(r) => {
                if r.status().is_success() {
                    let body = r.text().await.map_err(|e| format!("Read error: {}", e))?;
                    serde_json::from_str(&body).map_err(|e| format!("JSON parse error: {}", e))
                } else {
                    Err(format!("API error: {}", r.status()))
                }
            }
            Err(e) => Err(format!("Request failed: {}", e)),
        }
    }

    /// 批量获取多个代理节点的延迟
    #[allow(dead_code)]
    pub async fn get_proxies_delay(&self, proxy_names: &[String]) -> Vec<Option<u64>> {
        let mut result = vec![None; proxy_names.len()];

        // 使用 tokio 的 spawn 来并行测试，避免阻塞
        let tasks: Vec<_> = proxy_names
            .iter()
            .enumerate()
            .map(|(_i, name)| {
                let name_clone = name.clone();
                let client = self.client.clone();
                let api_url = self.api_url.clone();

                tokio::spawn(async move {
                    let encoded = percent_encode(&name_clone);
                    let url = format!(
                        "{}/proxies/{}/delay?url=http://www.gstatic.com/generate_204&timeout=3000",
                        api_url, encoded
                    );

                    match client.get(&url).send().await {
                        Ok(r) if r.status().is_success() => {
                            if let Ok(body) = r.text().await {
                                if let Ok(data) = serde_json::from_str::<serde_json::Value>(&body) {
                                    data.get("delay").and_then(|d| d.as_u64())
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        }
                        _ => None,
                    }
                })
            })
            .collect();

        // 等待所有任务完成
        for (i, task) in tasks.into_iter().enumerate() {
            if let Ok(delay) = task.await {
                result[i] = delay;
            }
        }

        result
    }
}

/// URL 路径百分号编码（用于 URL 路径部分，空格编码为 %20 而不是 +）
fn percent_encode(input: &str) -> String {
    let mut result = String::with_capacity(input.len() * 3);
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(byte as char);
            }
            _ => {
                result.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    result
}

/// 展开 ~ 为用户主目录
fn expand_tilde(path: &str) -> String {
    if path.starts_with("~/") || path == "~" {
        if let Some(home) = std::env::var("HOME").ok().or_else(|| std::env::var("USERPROFILE").ok()) {
            return path.replacen("~", &home, 1);
        }
    }
    path.to_string()
}
