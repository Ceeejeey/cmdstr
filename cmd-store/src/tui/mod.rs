mod app;
mod ui;

pub fn run() -> anyhow::Result<()> {
    app::App::new()?.run()
}
