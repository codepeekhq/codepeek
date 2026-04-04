use codepeek_view::App;
use color_eyre::Result;

fn main() -> Result<()> {
    color_eyre::install()?;

    let terminal = ratatui::init();
    let result = App::new().run(terminal);
    ratatui::restore();

    result?;
    Ok(())
}
