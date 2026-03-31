use crate::mihomo::MihomoController;
use crate::ui::Ui;
use crate::util::percent_encode;
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

// ---------------------------------------------------------------------------
// Delay test messaging — replaces the usize::MAX sentinel hack
// ---------------------------------------------------------------------------

#[derive(Debug)]
enum DelayMessage {
    /// Latency results for a specific proxy group.
    Results {
        group_index: usize,
        delays: Vec<Option<u64>>,
    },
    /// All-group refresh completed; carry the timestamp.
    RefreshDone(Instant),
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

pub struct App {
    pub controller: MihomoController,
    pub mode: String,
    pub selected_group: usize,
    pub selected_proxy: usize,
    /// (group_name, [node_names])
    pub proxies: Vec<(String, Vec<String>)>,
    /// Currently active node per group (from Mihomo `now` field).
    pub current_proxies: Vec<String>,
    /// Last group the user explicitly switched a node in.
    pub last_updated_group: Option<usize>,
    /// Per-group, per-node latency in ms. `None` = untested / timed-out.
    pub proxy_delays: Vec<Vec<Option<u64>>>,
    /// When the last full latency refresh completed.
    pub last_delay_test: Option<Instant>,
    /// Whether the Mihomo API is reachable.
    pub api_connected: bool,
    /// Last API error message, shown in the UI.
    pub api_error: Option<String>,
    pub should_quit: bool,
    delay_tx: mpsc::UnboundedSender<DelayMessage>,
    delay_rx: mpsc::UnboundedReceiver<DelayMessage>,
}

impl App {
    pub fn new(controller: MihomoController) -> Self {
        let (delay_tx, delay_rx) = mpsc::unbounded_channel();

        Self {
            controller,
            mode: "rule".to_string(),
            selected_group: 0,
            selected_proxy: 0,
            proxies: Vec::new(),
            current_proxies: Vec::new(),
            last_updated_group: None,
            proxy_delays: Vec::new(),
            last_delay_test: None,
            api_connected: false,
            api_error: None,
            should_quit: false,
            delay_tx,
            delay_rx,
        }
    }

    pub async fn run(
        &mut self,
        terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut last_tick = Instant::now();
        let tick_rate = Duration::from_millis(250);

        loop {
            terminal.draw(|f| Ui::draw(f, self))?;

            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or(Duration::ZERO);

            if crossterm::event::poll(timeout)? {
                if let Event::Key(key) = event::read()? {
                    self.handle_key(key);
                }
            }

            if last_tick.elapsed() >= tick_rate {
                self.update().await;
                last_tick = Instant::now();
            }

            if self.should_quit {
                return Ok(());
            }
        }
    }

    async fn update(&mut self) {
        // --- 1. Fetch config (mode) and proxy list in one API call each tick ---
        match self.controller.get_config().await {
            Ok(config) => {
                self.mode = config.mode;
                self.api_connected = true;
                self.api_error = None;
            }
            Err(e) => {
                self.api_connected = false;
                self.api_error = Some(e);
                // Drain any pending delay results and return early — no point
                // updating proxy state when the API is down.
                while self.delay_rx.try_recv().is_ok() {}
                return;
            }
        }

        // --- 2. Proxy list: fetch once on init, then only refresh `current_proxies` ---
        match self.controller.get_proxies().await {
            Ok(proxies_data) => {
                if self.proxies.is_empty() {
                    // Initial population
                    for (name, data) in proxies_data.iter() {
                        let type_str = data
                            .get("type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");

                        if matches!(type_str, "Selector" | "URLTest" | "Fallback") {
                            if let Some(all) = data.get("all").and_then(|v| v.as_array()) {
                                let nodes: Vec<String> = all
                                    .iter()
                                    .filter_map(|v| v.as_str().map(String::from))
                                    .collect();

                                let current = data
                                    .get("now")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();

                                self.proxy_delays.push(vec![None; nodes.len()]);
                                self.current_proxies.push(current);
                                self.proxies.push((name.clone(), nodes));
                            }
                        }
                    }

                    // Kick off latency test for the first group after init.
                    if !self.proxies.is_empty() {
                        self.start_delay_test(self.selected_group);
                        self.last_delay_test = Some(Instant::now());
                    }
                } else {
                    // Subsequent ticks: only update the active node per group.
                    for (i, (name, _)) in self.proxies.iter().enumerate() {
                        if let Some(data) = proxies_data.get(name) {
                            if let Some(now) = data.get("now").and_then(|v| v.as_str()) {
                                if let Some(slot) = self.current_proxies.get_mut(i) {
                                    *slot = now.to_string();
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                self.api_error = Some(e);
            }
        }

        // --- 3. Periodic latency refresh (every 60 s) ---
        if self
            .last_delay_test
            .map_or(false, |t| t.elapsed() > Duration::from_secs(60))
        {
            self.refresh_all_delays();
        }

        // --- 4. Drain delay results from background tasks ---
        while let Ok(msg) = self.delay_rx.try_recv() {
            match msg {
                DelayMessage::Results {
                    group_index,
                    delays,
                } => {
                    if let Some(slot) = self.proxy_delays.get_mut(group_index) {
                        *slot = delays;
                    }
                }
                DelayMessage::RefreshDone(ts) => {
                    self.last_delay_test = Some(ts);
                }
            }
        }

        // --- 5. Bounds-check cursor positions ---
        if !self.proxies.is_empty() && self.selected_group >= self.proxies.len() {
            self.selected_group = 0;
            self.selected_proxy = 0;
        }
        if let Some((_, nodes)) = self.proxies.get(self.selected_group) {
            if self.selected_proxy >= nodes.len() {
                self.selected_proxy = 0;
            }
        }
    }

    // -----------------------------------------------------------------------
    // Background latency helpers
    // -----------------------------------------------------------------------

    fn start_delay_test(&self, group_index: usize) {
        if let Some((_, nodes)) = self.proxies.get(group_index) {
            let nodes = nodes.clone();
            let tx = self.delay_tx.clone();
            let api_url = self.controller.api_url.clone();

            tokio::spawn(async move {
                let delays = test_delays(&api_url, &nodes).await;
                let _ = tx.send(DelayMessage::Results {
                    group_index,
                    delays,
                });
            });
        }
    }

    fn refresh_all_delays(&self) {
        let tx = self.delay_tx.clone();

        for (group_index, (_, nodes)) in self.proxies.iter().enumerate() {
            let nodes = nodes.clone();
            let tx2 = tx.clone();
            let api_url = self.controller.api_url.clone();

            tokio::spawn(async move {
                let delays = test_delays(&api_url, &nodes).await;
                let _ = tx2.send(DelayMessage::Results {
                    group_index,
                    delays,
                });
            });
        }

        // Signal that the refresh batch was dispatched.
        let _ = tx.send(DelayMessage::RefreshDone(Instant::now()));
    }

    // -----------------------------------------------------------------------
    // Input handling
    // -----------------------------------------------------------------------

    fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') => {
                self.should_quit = true;
            }

            // Space — switch to the highlighted node
            KeyCode::Char(' ') => {
                if let Some((group, nodes)) = self.proxies.get(self.selected_group) {
                    if let Some(node) = nodes.get(self.selected_proxy) {
                        let group = group.clone();
                        let node = node.clone();
                        let controller = self.controller.clone();
                        tokio::spawn(async move {
                            let _ = controller.select_proxy(&group, &node).await;
                        });
                        self.last_updated_group = Some(self.selected_group);
                    }
                }
            }

            // Mode switching
            KeyCode::Char('r') => {
                let c = self.controller.clone();
                tokio::spawn(async move { let _ = c.switch_mode("rule").await; });
            }
            KeyCode::Char('g') => {
                let c = self.controller.clone();
                tokio::spawn(async move { let _ = c.switch_mode("global").await; });
            }
            KeyCode::Char('d') => {
                let c = self.controller.clone();
                tokio::spawn(async move { let _ = c.switch_mode("direct").await; });
            }

            // Tab — cycle proxy groups
            KeyCode::Tab => {
                if !self.proxies.is_empty() {
                    self.selected_group = (self.selected_group + 1) % self.proxies.len();
                    self.selected_proxy = 0;
                    self.start_delay_test(self.selected_group);
                }
            }

            // f — refresh all latencies
            KeyCode::Char('f') => {
                self.refresh_all_delays();
            }

            // j/k — vim-style node navigation
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some((_, nodes)) = self.proxies.get(self.selected_group) {
                    if !nodes.is_empty() {
                        self.selected_proxy = (self.selected_proxy + 1) % nodes.len();
                    }
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some((_, nodes)) = self.proxies.get(self.selected_group) {
                    if !nodes.is_empty() {
                        self.selected_proxy = if self.selected_proxy == 0 {
                            nodes.len() - 1
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

// ---------------------------------------------------------------------------
// Standalone async helper (no &self borrow needed in spawned tasks)
// ---------------------------------------------------------------------------

async fn test_delays(api_url: &str, nodes: &[String]) -> Vec<Option<u64>> {
    let client = reqwest::Client::builder()
        .no_proxy()
        .build()
        .unwrap();

    let tasks: Vec<_> = nodes
        .iter()
        .map(|name| {
            let name = name.clone();
            let api_url = api_url.to_string();
            let client = client.clone();

            tokio::spawn(async move {
                let encoded = percent_encode(&name);
                let url = format!(
                    "{}/proxies/{}/delay?url=http://www.gstatic.com/generate_204&timeout=3000",
                    api_url, encoded
                );
                match client.get(&url).send().await {
                    Ok(r) if r.status().is_success() => {
                        let body = r.text().await.ok()?;
                        let data: serde_json::Value = serde_json::from_str(&body).ok()?;
                        data.get("delay").and_then(|d| d.as_u64())
                    }
                    _ => None,
                }
            })
        })
        .collect();

    let mut results = vec![None; nodes.len()];
    for (i, task) in tasks.into_iter().enumerate() {
        if let Ok(delay) = task.await {
            results[i] = delay;
        }
    }
    results
}
