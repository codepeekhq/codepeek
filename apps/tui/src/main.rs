use codepeek_git::GitChangeDetector;
use codepeek_syntax::TreeSitter;
use codepeek_view::App;
use color_eyre::Result;

fn main() -> Result<()> {
    color_eyre::install()?;

    let repo_path = std::env::current_dir()?;
    let detector = GitChangeDetector::open(&repo_path)?;
    let highlighter = TreeSitter::new();

    let terminal = ratatui::init();
    let result = App::new(Box::new(detector), Box::new(highlighter))?.run(terminal);
    ratatui::restore();

    result?;
    Ok(())
}
