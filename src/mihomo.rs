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
            mihomo_path: shellexpand::tilde(mihomo_path).to_string(),
            config_path: shellexpand::tilde(config_path).to_string(),
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
                    #[derive(Deserialize)]
                    struct ProxiesResponse {
                        proxies: HashMap<String, serde_json::Value>,
                    }
                    let data: ProxiesResponse = r.json().await.map_err(|e| format!("JSON parse error: {}", e))?;
                    Ok(data.proxies)
                } else {
                    Err(format!("API error: {}", r.status()))
                }
            }
            Err(e) => Err(format!("Request failed: {}", e)),
        }
    }

    pub async fn select_proxy(&self, group: &str, proxy: &str) -> Result<(), String> {
        let url = format!("{}/proxies/{}", self.api_url, group);
        let body = serde_json::json!({ "name": proxy });
        let resp = self.client.put(&url).json(&body).send().await;

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
        let body = serde_json::json!({ "mode": mode });
        let resp = self.client.patch(&url).json(&body).send().await;

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
                    r.json().await.map_err(|e| format!("JSON parse error: {}", e))
                } else {
                    Err(format!("API error: {}", r.status()))
                }
            }
            Err(e) => Err(format!("Request failed: {}", e)),
        }
    }

    /// 获取指定代理节点的延迟
    /// 返回延迟（毫秒），如果测试超时或失败返回 None
    #[allow(dead_code)]
    pub async fn get_proxy_delay(&self, proxy_name: &str) -> Option<u64> {
        // URL 编码节点名称
        let encoded_name = urlencoding::encode(proxy_name);
        let url = format!(
            "{}/proxies/{}/delay?url=http://www.gstatic.com/generate_204&timeout=3000",
            self.api_url, encoded_name
        );
        let resp = self.client.get(&url).send().await;

        match resp {
            Ok(r) => {
                if r.status().is_success() {
                    #[derive(Deserialize)]
                    struct DelayResponse {
                        delay: Option<u64>,
                    }
                    if let Ok(data) = r.json::<DelayResponse>().await {
                        data.delay
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            Err(_) => None,
        }
    }

    /// 批量获取多个代理节点的延迟
    #[allow(dead_code)]
    pub async fn get_proxies_delay(&self, proxy_names: &[String]) -> Vec<Option<u64>> {
        let mut futures = Vec::new();
        for name in proxy_names {
            let name_clone = name.clone();
            let client = self.client.clone();
            let api_url = self.api_url.clone();

            let future = async move {
                // URL 编码节点名称
                let encoded_name = urlencoding::encode(&name_clone);
                let url = format!(
                    "{}/proxies/{}/delay?url=http://www.gstatic.com/generate_204&timeout=3000",
                    api_url, encoded_name
                );
                let resp = client.get(&url).send().await;

                match resp {
                    Ok(r) => {
                        if r.status().is_success() {
                            #[derive(Deserialize)]
                            struct DelayResponse {
                                delay: Option<u64>,
                            }
                            if let Ok(data) = r.json::<DelayResponse>().await {
                                data.delay
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    }
                    Err(_) => None,
                }
            };
            futures.push(future);
        }

        let results = futures::future::join_all(futures).await;
        results.into_iter().map(|r| r).collect()
    }
}
