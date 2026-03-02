# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

mihomo-tui 是一个使用 Rust 和 ratatui 构建的终端用户界面（TUI）应用程序，用于管理和控制 Mihomo 代理服务。

## 常用命令

- `cargo build` - 构建项目
- `cargo run` - 运行 TUI 应用
- `cargo check` - 快速检查代码（不构建）
- `cargo clippy` - 运行 Rust linter
- `cargo fmt` - 格式化代码

## 架构

项目采用经典的 TUI 应用架构，分为四个主要模块：

### 模块结构

- **`main.rs`** - 应用入口。负责初始化终端（raw mode、alternate screen）、创建 `MihomoController` 和 `App` 实例，以及处理终端恢复。

- **`app.rs`** - 核心应用逻辑。包含事件循环、状态管理和键盘事件处理。
  - 250ms 的 tick rate 用于定期更新
  - 状态包括：当前模式（rule/global/direct）、选中的代理组和节点、代理列表
  - 键盘绑定：`q` 退出、`s` 切换节点、`r/g/d` 切换模式、方向键选择

- **`ui.rs`** - 使用 ratatui 的 UI 渲染逻辑。
  - 布局：header（显示当前模式）、main（左右分栏显示组和节点）、footer（帮助信息）
  - 选中的项目和节点以绿色高亮显示

- **`mihomo.rs`** - Mihomo REST API 客户端。
  - API 地址：`127.0.0.1:9090`
  - 使用 `reqwest::Client::builder().no_proxy()` 确保请求不被 mihomo 代理
  - 主要方法：`get_proxies()`、`select_proxy()`、`switch_mode()`、`get_config()`

### 关键设计细节

1. **代理组过滤**：只显示 `Selector`、`URLTest` 和 `Fallback` 类型的代理组
2. **更新策略**：代理列表仅在初始化时获取（避免频繁刷新导致闪烁），模式状态每 250ms 更新一次
3. **路径展开**：使用 `shellexpand::tilde` 处理 `~` 路径
4. **默认配置**：mihomo 路径 `~/mihomo-tui/bin/mihomo`，配置路径 `~/.config/mihomo/config.yaml`

## 依赖说明

- `ratatui` - TUI 框架
- `crossterm` - 终端操作和事件处理
- `tokio` - 异步运行时
- `reqwest` - HTTP 客户端，使用 `rustls-tls` 避免系统依赖
- `serde/serde_json` - JSON 序列化
