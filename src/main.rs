use clap::Parser;
use std::path::PathBuf;

use termstack::{
    app::App,
    config::{ConfigLoader, ConfigValidator},
    globals,
};

#[derive(Parser)]
#[command(name = "termstack")]
#[command(about = "A generic TUI framework for building dashboards from YAML config", long_about = None)]
struct Cli {
    /// Path to the YAML configuration file
    #[arg(value_name = "CONFIG")]
    config: PathBuf,

    /// Validate config and exit (don't run TUI)
    #[arg(long)]
    validate: bool,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let cli = Cli::parse();

    // Load config
    println!("Loading config from: {:?}", cli.config);
    let config = match ConfigLoader::load_from_file(&cli.config) {
        Ok(cfg) => {
            println!("✓ Config loaded successfully");
            cfg
        }
        Err(e) => {
            eprintln!("✗ Failed to load config: {}", e);
            eprintln!("\nError details: {:?}", e);
            std::process::exit(1);
        }
    };

    // Validate config
    println!("Validating config...");
    if let Err(e) = ConfigValidator::validate(&config) {
        eprintln!("✗ Config validation failed: {}", e);
        eprintln!("\nFull error chain:");
        for cause in e.chain() {
            eprintln!("  - {}", cause);
        }
        std::process::exit(1);
    }
    println!("✓ Config is valid");

    // If validate-only mode, exit here
    if cli.validate {
        println!("\n✓ Configuration is valid!");
        return Ok(());
    }

    // Initialize globals
    globals::init_config(config.clone())
        .map_err(|e| color_eyre::eyre::eyre!("Failed to initialize config: {}", e))?;
    globals::init_template_engine()
        .map_err(|e| color_eyre::eyre::eyre!("Failed to initialize template engine: {}", e))?;

    // Show config summary
    if cli.verbose {
        println!("\nConfig Summary:");
        println!("  App: {}", config.app.name);
        println!("  Start page: {}", config.start);
        println!("  Pages: {}", config.pages.len());
        for (page_id, page) in &config.pages {
            println!("    - {} ({})", page_id, page.title);
        }
        println!();
    }

    // Initialize adapter registry with default adapters
    let adapter_registry = termstack::adapters::registry::AdapterRegistry::with_defaults();

    // Run TUI
    println!("Starting TUI...\n");
    let terminal = ratatui::init();
    let app = App::new(config, adapter_registry).map_err(|e| color_eyre::eyre::eyre!("{}", e))?;
    let result = app
        .run(terminal)
        .await
        .map_err(|e| color_eyre::eyre::eyre!("{}", e));
    ratatui::restore();
    result
}
