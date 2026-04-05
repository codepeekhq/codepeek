mod config;

use codepeek_git::GitChangeDetector;
use codepeek_syntax::TreeSitter;
use codepeek_view::App;
use color_eyre::Result;

fn main() -> Result<()> {
    color_eyre::install()?;

    let (app_config, config_warning) = config::AppConfig::load();

    // Print config warnings before TUI takes over the terminal.
    // These are non-fatal (defaults are used) but the user should know.
    if let Some(warning) = config_warning {
        eprintln!("codepeek: {warning}");
    }

    let repo_path = std::env::current_dir()?;
    let detector = GitChangeDetector::open(&repo_path)?;
    let highlighter = TreeSitter::with_languages(app_config.enabled_languages());

    let terminal = ratatui::init();
    let result = App::new(Box::new(detector), Box::new(highlighter))?.run(terminal);
    ratatui::restore();

    result?;
    Ok(())
}
