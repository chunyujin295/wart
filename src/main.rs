// Deliberately not `windows_subsystem = "windows"`: this binary is both a GUI
// and a CLI, and the subsystem attribute would strip the console from the CLI
// mode, breaking `wart --text ... > out.txt`. The cost is a console window
// alongside the GUI in release builds.
mod app;
mod assets;
mod cli;
mod color;
mod export;
mod fonts;
mod i18n;
mod model;
mod nerd;
mod render;
mod ui;

fn main() -> anyhow::Result<()> {
    cli::run()
}
