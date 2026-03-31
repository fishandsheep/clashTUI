mod app;
mod mihomo;
mod ui;
mod util;

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;

use app::App;
use mihomo::MihomoController;

fn print_usage() {
    eprintln!("Usage: mihomo-tui [OPTIONS]");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  --api <host:port>   Mihomo API address (default: 127.0.0.1:9090)");
    eprintln!("  --secret <token>    Mihomo API secret for authentication");
    eprintln!("  --help              Show this help message");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut api_addr = "127.0.0.1:9090".to_string();
    let mut secret: Option<String> = None;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--api" => {
                api_addr = args.next().unwrap_or_else(|| {
                    eprintln!("Error: --api requires a value");
                    std::process::exit(1);
                });
            }
            "--secret" => {
                secret = Some(args.next().unwrap_or_else(|| {
                    eprintln!("Error: --secret requires a value");
                    std::process::exit(1);
                }));
            }
            "--help" | "-h" => {
                print_usage();
                return Ok(());
            }
            unknown => {
                eprintln!("Unknown argument: {}", unknown);
                print_usage();
                std::process::exit(1);
            }
        }
    }

    let api_url = format!("http://{}", api_addr);
    let controller = MihomoController::new(&api_url, secret.as_deref());
    let mut app = App::new(controller);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = app.run(&mut terminal).await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("{:?}", err);
    }

    Ok(())
}
