mod app;
mod backend;
mod config;
mod system;
mod ui;
mod utils;

use anyhow::Result;
use clap::{Parser, Subcommand};
use env_logger::Env;
use gtk4::prelude::*;
use gtk4::{Application};

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

    // Initialize logging
    let log_level = if cli.debug { "debug" } else { "info" };
    env_logger::Builder::from_env(Env::default().default_filter_or(log_level)).init();

    log::info!("Starting CPU Power Manager v{}", env!("CARGO_PKG_VERSION"));

    // Handle CLI commands
    if let Some(command) = cli.command {
        return handle_cli_command(command);
    }

    // Start GTK application
    let app = Application::builder().application_id(APP_ID).build();

    app.connect_startup(|_| {
        log::info!("Application startup");
    });

    app.connect_activate(move |app| {
        setup_css(); // Load CSS here to ensure it's applied before the window is built

        log::info!("Application activate");
        let window = app::AppWindow::new(app);

        if !cli.minimized {
            window.present();
        }
    });

    // app.run() returns glib::ExitCode, not ()
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
            println!("  Turbo: {}", if cpu_manager.is_turbo_enabled()? { "Enabled" } else { "Disabled" });
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
            let profile = config_manager.get_profile(&name)?;
            profile.apply(&cpu_manager)?;
            println!("Profile '{}' applied", name);
        }
        Commands::Service => {
            log::info!("Starting background service");
            // TODO: Implement service mode with auto-tuning
            println!("Service mode not yet implemented");
        }
        Commands::Version => {
            println!("CPU Power Manager v{}", env!("CARGO_PKG_VERSION"));
            println!("Built with Rust {}", env!("CARGO_PKG_RUST_VERSION"));
        }
    }

    Ok(())
}

fn setup_css() {
    use gtk4::gdk::Display;
    use gtk4::CssProvider;

    let provider = CssProvider::new();

    // Load Dracula theme CSS with traffic light styles
    let css = include_str!("../resources/style.css");
    provider.load_from_data(css);

    gtk4::style_context_add_provider_for_display(
        &Display::default().expect("Could not connect to a display"),
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}
