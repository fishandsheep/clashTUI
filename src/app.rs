use crate::mihomo::MihomoController;
use crate::ui::Ui;
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use std::time::Duration;

/// 代理组类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProxyType {
    Selector,   // 可手动选择
    UrlTest,    // 自动测试选择
    Fallback,   // 自动故障转移
}

impl ProxyType {
    /// 获取类型的显示标记
    pub fn marker(&self) -> &'static str {
        match self {
            ProxyType::Selector => "[S]",      // 可手动切换
            ProxyType::UrlTest => "[A]",       // 自动
            ProxyType::Fallback => "[F]",      // 故障转移
        }
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
    pub should_quit: bool,
}

impl App {
    pub fn new(controller: MihomoController) -> Self {
        Self {
            controller,
            mode: "rule".to_string(),
            selected_group: 0,
            selected_proxy: 0,
            proxies: Vec::new(),
            current_proxies: Vec::new(),
            proxy_types: Vec::new(),
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
                    self.handle_key(key).await;
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
                        if proxy_type_str == "Selector" || proxy_type_str == "URLTest" || proxy_type_str == "Fallback" {
                            if let Some(all) = data.get("all").and_then(|v| v.as_array()) {
                                let proxy_names: Vec<String> = all
                                    .iter()
                                    .filter_map(|v| v.as_str().map(String::from))
                                    .collect();
                                self.proxies.push((name.clone(), proxy_names));

                                // 存储代理组类型
                                let proxy_type = match proxy_type_str {
                                    "Selector" => ProxyType::Selector,
                                    "URLTest" => ProxyType::UrlTest,
                                    "Fallback" => ProxyType::Fallback,
                                    _ => ProxyType::Selector,
                                };
                                self.proxy_types.push(proxy_type);

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

    async fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') => {
                self.should_quit = true;
            }
            KeyCode::Char('s') => {
                // 只有 Selector 类型的代理组才允许手动切换
                if let Some(proxy_type) = self.proxy_types.get(self.selected_group) {
                    if *proxy_type == ProxyType::Selector {
                        if let Some((group, proxies)) = self.proxies.get(self.selected_group) {
                            if let Some(proxy) = proxies.get(self.selected_proxy) {
                                let _ = self.controller.select_proxy(group, proxy).await;
                            }
                        }
                    }
                }
            }
            KeyCode::Char('r') => {
                let _ = self.controller.switch_mode("rule").await;
            }
            KeyCode::Char('g') => {
                let _ = self.controller.switch_mode("global").await;
            }
            KeyCode::Char('d') => {
                let _ = self.controller.switch_mode("direct").await;
            }
            KeyCode::Up => {
                if self.selected_group > 0 {
                    self.selected_group -= 1;
                    self.selected_proxy = 0;
                }
            }
            KeyCode::Down => {
                if !self.proxies.is_empty() && self.selected_group < self.proxies.len() - 1 {
                    self.selected_group += 1;
                    self.selected_proxy = 0;
                }
            }
            KeyCode::Left => {
                if let Some((_, _proxies)) = self.proxies.get(self.selected_group) {
                    if self.selected_proxy > 0 {
                        self.selected_proxy -= 1;
                    }
                }
            }
            KeyCode::Right => {
                if let Some((_, _proxies)) = self.proxies.get(self.selected_group) {
                    if self.selected_proxy < self.proxies[self.selected_group].1.len().saturating_sub(1) {
                        self.selected_proxy += 1;
                    }
                }
            }
            _ => {}
        }
    }
}
