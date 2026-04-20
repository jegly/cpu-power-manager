mod app;
mod backend;
mod config;
mod system;
mod ui;
mod utils;
mod tray;

use anyhow::Result;
use clap::{Parser, Subcommand};
use env_logger::Env;
use gtk4::prelude::*;
use gtk4::Application;

const APP_ID: &str = "com.cpupowermanager.App";

#[derive(Parser)]
#[command(name = "cpu-power-manager")]
#[command(about = "Advanced CPU Power Management Tool", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,

    /// Start minimized to system tray
    #[arg(short, long)]
    minimized: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Show current CPU status
    Status,
    /// Set CPU governor
    SetGovernor { governor: String },
    /// Set CPU frequency (in MHz)
    SetFrequency { frequency: u32 },
    /// Enable/disable turbo boost
    SetTurbo { enabled: bool },
    /// Apply a profile
    ApplyProfile { name: String },
    /// Start the background service
    Service,
    /// Show version information
    Version,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let log_level = if cli.debug { "debug" } else { "info" };
    env_logger::Builder::from_env(Env::default().default_filter_or(log_level)).init();

    log::info!("Starting CPU Power Manager v{}", env!("CARGO_PKG_VERSION"));

    if let Some(command) = cli.command {
        return handle_cli_command(command);
    }

    let app = Application::builder().application_id(APP_ID).build();

    app.connect_startup(|_| {
        log::info!("Application startup");
    });

    let minimized = cli.minimized;
    app.connect_activate(move |app| {
        setup_css();
        // Apply saved UI scale
        if let Ok(cfg_mgr) = config::ConfigManager::new() {
            apply_ui_scale(&cfg_mgr.get_config().general.ui_scale.clone());
        }
        log::info!("Application activate");
        let window = app::AppWindow::new(app);

        // Spawn system tray — fails gracefully if no DBus session bus (e.g. sudo without -E)
        tray::spawn(window.window_handle());

        if !minimized {
            window.present();
        }
    });

    app.run();
    Ok(())
}

fn handle_cli_command(command: Commands) -> Result<()> {
    use backend::cpu::CpuManager;

    let cpu_manager = CpuManager::new()?;

    match command {
        Commands::Status => {
            println!("CPU Status:");
            let info = cpu_manager.get_cpu_info()?;
            println!("  Model: {}", info.model);
            println!("  Cores: {}", info.core_count);
            println!("  Governor: {}", cpu_manager.get_governor(0)?);
            println!("  Frequencies:");
            for (core, freq) in cpu_manager.get_all_frequencies()?.iter().enumerate() {
                println!("    Core {}: {} MHz", core, freq);
            }
            println!(
                "  Turbo: {}",
                if cpu_manager.is_turbo_enabled()? { "Enabled" } else { "Disabled" }
            );
        }
        Commands::SetGovernor { governor } => {
            cpu_manager.set_governor_all(&governor)?;
            println!("Governor set to: {}", governor);
        }
        Commands::SetFrequency { frequency } => {
            cpu_manager.set_frequency_all(frequency)?;
            println!("Frequency set to: {} MHz", frequency);
        }
        Commands::SetTurbo { enabled } => {
            cpu_manager.set_turbo(enabled)?;
            println!("Turbo boost: {}", if enabled { "Enabled" } else { "Disabled" });
        }
        Commands::ApplyProfile { name } => {
            let config_manager = config::ConfigManager::new()?;
            // FIX: get_profile returned Option, not Result; now returns Result in config/mod.rs
            let profile = config_manager.get_profile(&name)?;
            profile.apply(&cpu_manager)?;
            println!("Profile '{}' applied", name);
        }
        Commands::Service => {
            log::info!("Starting background service");
            println!("Service mode not yet implemented");
        }
        Commands::Version => {
            println!("CPU Power Manager v{}", env!("CARGO_PKG_VERSION"));
            // FIX: CARGO_PKG_RUST_VERSION is the *minimum* required version from Cargo.toml,
            // not the compiler version. Use CARGO_PKG_RUST_VERSION if set, else omit.
            println!("Built with Rust (see rustc --version)");
        }
    }

    Ok(())
}

thread_local! {
    static SCALE_PROVIDER: std::cell::RefCell<Option<gtk4::CssProvider>> = std::cell::RefCell::new(None);
}

pub fn apply_ui_scale(scale: &str) {
    use gtk4::gdk::Display;
    use gtk4::prelude::*;
    let display = match Display::default() { Some(d) => d, None => return };
    SCALE_PROVIDER.with(|cell| {
        if let Some(old) = cell.borrow().as_ref() {
            gtk4::style_context_remove_provider_for_display(&display, old);
        }
    });
    let css = match scale {
        "small"  => "label{font-size:11px;}.value{font-size:14px;}.freq-value,.usage-value{font-size:16px;}.temp-normal,.temp-warm,.temp-hot,.temp-critical{font-size:14px;}",
        "large"  => "label{font-size:15px;}.value{font-size:19px;}.freq-value,.usage-value{font-size:24px;}.temp-normal,.temp-warm,.temp-hot,.temp-critical{font-size:22px;}",
        "xlarge" => "label{font-size:18px;}.value{font-size:22px;}.freq-value,.usage-value{font-size:28px;}.temp-normal,.temp-warm,.temp-hot,.temp-critical{font-size:26px;}",
        _ => { SCALE_PROVIDER.with(|c| *c.borrow_mut() = None); return; }
    };
    let provider = gtk4::CssProvider::new();
    provider.load_from_data(css);
    gtk4::style_context_add_provider_for_display(
        &display, &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
    );
    SCALE_PROVIDER.with(|c| *c.borrow_mut() = Some(provider));
}

fn setup_css() {
    use gtk4::gdk::Display;
    use gtk4::CssProvider;

    let display = Display::default().expect("Could not connect to a display");

    let provider = CssProvider::new();
    let css = include_str!("../resources/style.css");
    provider.load_from_data(css);

    gtk4::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    // Register the bundled icon so it works without system installation.
    // The icon lives at assets/hicolor/scalable/apps/cpu-power-manager.svg;
    // we add the assets/ directory as a theme search root.
    let theme = gtk4::IconTheme::for_display(&display);
    // Try CARGO_MANIFEST_DIR first (cargo run), then walk up from the binary.
    let found = std::env::var("CARGO_MANIFEST_DIR")
        .ok()
        .map(std::path::PathBuf::from)
        .filter(|p| p.join("assets/hicolor").exists())
        .or_else(|| {
            std::env::current_exe().ok().and_then(|mut exe| {
                for _ in 0..5 {
                    exe.pop();
                    if exe.join("assets/hicolor").exists() {
                        return Some(exe);
                    }
                }
                None
            })
        });
    if let Some(root) = found {
        theme.add_search_path(root.join("assets"));
    }
}
