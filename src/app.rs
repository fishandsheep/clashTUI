use crate::mihomo::MihomoController;
use crate::ui::Ui;
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

/// 延迟测试结果消息
#[derive(Debug)]
struct DelayTestResult {
    group_index: usize,
    delays: Vec<Option<u64>>,
}

/// 代理组类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProxyType {
    Selector,   // 可手动选择
}

impl ProxyType {
    /// 获取类型的显示标记
    pub fn marker(&self) -> &'static str {
        "[S]"  // 可手动切换
    }
}

pub struct App {
    pub controller: MihomoController,
    pub mode: String,
    pub selected_group: usize,
    pub selected_proxy: usize,
    pub proxies: Vec<(String, Vec<String>)>,
    /// 每个代理组当前选中的节点名称
    pub current_proxies: Vec<String>,
    /// 每个代理组的类型
    pub proxy_types: Vec<ProxyType>,
    /// 最后操作/切换过的组索引（用于标识"生效"的组）
    pub last_updated_group: Option<usize>,
    /// 每个代理组中各节点的延迟（毫秒），None 表示未测试或超时
    pub proxy_delays: Vec<Vec<Option<u64>>>,
    /// 上次延迟测试时间
    pub last_delay_test: Option<Instant>,
    /// 延迟测试结果接收器
    delay_result_rx: mpsc::UnboundedReceiver<DelayTestResult>,
    /// 延迟测试结果发送器
    delay_result_tx: mpsc::UnboundedSender<DelayTestResult>,
    pub should_quit: bool,
}

impl App {
    pub fn new(controller: MihomoController) -> Self {
        let (delay_result_tx, delay_result_rx) = mpsc::unbounded_channel();

        Self {
            controller,
            mode: "rule".to_string(),
            selected_group: 0,
            selected_proxy: 0,
            proxies: Vec::new(),
            current_proxies: Vec::new(),
            proxy_types: Vec::new(),
            last_updated_group: None,
            proxy_delays: Vec::new(),
            last_delay_test: None,
            delay_result_rx,
            delay_result_tx,
            should_quit: false,
        }
    }

    pub async fn run(&mut self, terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>) -> Result<(), Box<dyn std::error::Error>> {
        let mut last_tick = std::time::Instant::now();
        let tick_rate = Duration::from_millis(250);

        loop {
            terminal.draw(|f| Ui::draw(f, self))?;

            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));

            if crossterm::event::poll(timeout)? {
                if let Event::Key(key) = event::read()? {
                    self.handle_key(key);
                }
            }

            if last_tick.elapsed() >= tick_rate {
                self.update().await;
                last_tick = std::time::Instant::now();
            }

            if self.should_quit {
                return Ok(());
            }
        }
    }

    async fn update(&mut self) {
        // 只更新模式，不频繁更新代理列表（避免闪烁）
        if let Ok(config) = self.controller.get_config().await {
            self.mode = config.mode;
        }

        // 只在代理列表为空时才获取（初始化时）
        if self.proxies.is_empty() {
            if let Ok(proxies_data) = self.controller.get_proxies().await {
                for (name, data) in proxies_data.iter() {
                    if let Some(proxy_type_str) = data.get("type").and_then(|v| v.as_str()) {
                        if proxy_type_str == "Selector" {
                            if let Some(all) = data.get("all").and_then(|v| v.as_array()) {
                                let proxy_names: Vec<String> = all
                                    .iter()
                                    .filter_map(|v| v.as_str().map(String::from))
                                    .collect();
                                self.proxies.push((name.clone(), proxy_names.clone()));

                                // 存储代理组类型（只可能是 Selector）
                                self.proxy_types.push(ProxyType::Selector);

                                // 初始化延迟数据
                                self.proxy_delays.push(vec![None; proxy_names.len()]);

                                // 获取当前选中的节点
                                if let Some(now) = data.get("now").and_then(|v| v.as_str()) {
                                    self.current_proxies.push(now.to_string());
                                } else {
                                    self.current_proxies.push(String::new());
                                }
                            }
                        }
                    }
                }

                // 初始化完成后，异步测试第一个组的延迟
                if !self.proxies.is_empty() {
                    self.start_delay_test(self.selected_group);
                }
            }
        }

        // 定期更新当前选中的节点（用于显示选择结果）
        if !self.proxies.is_empty() {
            if let Ok(proxies_data) = self.controller.get_proxies().await {
                for (i, (name, _)) in self.proxies.iter().enumerate() {
                    if let Some(data) = proxies_data.get(name) {
                        if let Some(now) = data.get("now").and_then(|v| v.as_str()) {
                            if i < self.current_proxies.len() {
                                self.current_proxies[i] = now.to_string();
                            }
                        }
                    }
                }
            }
        }

        // 每 60 秒测试一次当前选中组的节点延迟
        if self.last_delay_test.is_none() {
            // 首次测试已在初始化后触发
            self.last_delay_test = Some(Instant::now());
        } else if self.last_delay_test.map_or(false, |t| t.elapsed() > Duration::from_secs(60)) {
            self.start_delay_test(self.selected_group);
            self.last_delay_test = Some(Instant::now());
        }

        // 检查是否有延迟测试结果返回
        while let Ok(result) = self.delay_result_rx.try_recv() {
            if result.group_index == usize::MAX {
                // 刷新信号，更新最后刷新时间
                self.last_delay_test = Some(Instant::now());
            } else if result.group_index < self.proxy_delays.len() {
                self.proxy_delays[result.group_index] = result.delays;
            }
        }

        if self.selected_group >= self.proxies.len() && !self.proxies.is_empty() {
            self.selected_group = 0;
            self.selected_proxy = 0;
        }

        if let Some((_, proxies)) = self.proxies.get(self.selected_group) {
            if self.selected_proxy >= proxies.len() {
                self.selected_proxy = 0;
            }
        }
    }

    /// 启动异步延迟测试（不阻塞）
    fn start_delay_test(&self, group_index: usize) {
        if let Some((_group_name, proxy_names)) = self.proxies.get(group_index) {
            let proxy_names = proxy_names.clone();
            let tx = self.delay_result_tx.clone();
            let api_url = self.controller.api_url.clone();

            // 在后台异步执行延迟测试
            tokio::spawn(async move {
                let delays = test_delays_impl(&api_url, &proxy_names).await;
                let _ = tx.send(DelayTestResult {
                    group_index,
                    delays,
                });
            });
        }
    }

    /// 刷新所有代理组的延迟测试
    fn refresh_all_delays(&self) {
        let tx = self.delay_result_tx.clone();

        for (group_index, (_group_name, proxy_names)) in self.proxies.iter().enumerate() {
            let proxy_names = proxy_names.clone();
            let tx_clone = tx.clone();
            let api_url = self.controller.api_url.clone();
            let idx = group_index;

            tokio::spawn(async move {
                let delays = test_delays_impl(&api_url, &proxy_names).await;
                let _ = tx_clone.send(DelayTestResult {
                    group_index: idx,
                    delays,
                });
            });
        }

        // 更新最后刷新时间
        let _ = tx.send(DelayTestResult {
            group_index: usize::MAX, // 标记为刷新信号
            delays: vec![],
        });
    }

    fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') => {
                self.should_quit = true;
            }
            KeyCode::Char(' ') => {
                // 空格键切换节点
                if let Some(proxy_type) = self.proxy_types.get(self.selected_group) {
                    if *proxy_type == ProxyType::Selector {
                        if let Some((group, proxies)) = self.proxies.get(self.selected_group) {
                            if let Some(proxy) = proxies.get(self.selected_proxy) {
                                // 这里不使用 await，让切换操作立即完成
                                let group = group.clone();
                                let proxy = proxy.clone();
                                let controller = self.controller.clone();
                                tokio::spawn(async move {
                                    let _ = controller.select_proxy(&group, &proxy).await;
                                });
                                // 记录最后操作的组
                                self.last_updated_group = Some(self.selected_group);
                            }
                        }
                    }
                }
            }
            KeyCode::Char('r') => {
                let controller = self.controller.clone();
                tokio::spawn(async move {
                    let _ = controller.switch_mode("rule").await;
                });
            }
            KeyCode::Char('g') => {
                let controller = self.controller.clone();
                tokio::spawn(async move {
                    let _ = controller.switch_mode("global").await;
                });
            }
            KeyCode::Char('d') => {
                let controller = self.controller.clone();
                tokio::spawn(async move {
                    let _ = controller.switch_mode("direct").await;
                });
            }
            // Tab 键循环切换代理组
            KeyCode::Tab => {
                if !self.proxies.is_empty() {
                    self.selected_group = (self.selected_group + 1) % self.proxies.len();
                    self.selected_proxy = 0;
                    // 切换组时启动异步延迟测试（不阻塞）
                    self.start_delay_test(self.selected_group);
                }
            }
            // f 键刷新所有组的延迟
            KeyCode::Char('f') => {
                self.refresh_all_delays();
            }
            // j/k 键上下移动节点（vim 风格）
            KeyCode::Char('j') => {
                if let Some((_, proxies)) = self.proxies.get(self.selected_group) {
                    if !proxies.is_empty() {
                        self.selected_proxy = (self.selected_proxy + 1) % proxies.len();
                    }
                }
            }
            KeyCode::Char('k') => {
                if let Some((_, proxies)) = self.proxies.get(self.selected_group) {
                    if !proxies.is_empty() {
                        self.selected_proxy = if self.selected_proxy == 0 {
                            proxies.len() - 1
                        } else {
                            self.selected_proxy - 1
                        };
                    }
                }
            }
            _ => {}
        }
    }
}

/// 延迟测试的实际实现（在后台任务中执行）
async fn test_delays_impl(api_url: &str, proxy_names: &[String]) -> Vec<Option<u64>> {
    let client = reqwest::Client::builder()
        .no_proxy()
        .build()
        .unwrap();

    // 创建所有任务
    let tasks: Vec<_> = proxy_names
        .iter()
        .map(|name| {
            let name_clone = name.clone();
            let api_url = api_url.to_string();
            let client = client.clone();

            tokio::spawn(async move {
                let encoded = percent_encode_path(&name_clone);
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
    let mut results = vec![None; proxy_names.len()];
    for (i, task) in tasks.into_iter().enumerate() {
        if let Ok(delay) = task.await {
            results[i] = delay;
        }
    }

    results
}

/// URL 路径百分号编码（用于 URL 路径部分，空格编码为 %20 而不是 +）
fn percent_encode_path(input: &str) -> String {
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
