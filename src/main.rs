mod config;
mod discover;
#[cfg(windows)]
mod draft;
mod embed;
mod engine;
mod eval;
mod rank;
mod setup;
mod ui;
mod worker;

const USAGE: &str = "usage: skill-widget                 open the widget
       skill-widget install         add the prompt hook to ~/.claude/settings.json
       skill-widget uninstall       remove it
       skill-widget eval [--model NAME] [--file eval.toml]
       skill-widget models          list built-in fastembed model names
       skill-widget hook            (run by Claude Code, not by hand)";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        None => detach(),
        Some("gui") => ui::run(),
        Some("hook") => {
            setup::hook();
            Ok(())
        }
        Some("install") => setup::install(),
        Some("uninstall") => setup::uninstall(),
        Some("eval") => eval::run(&args[1..]),
        Some("models") => {
            for m in fastembed::TextEmbedding::list_supported_models() {
                println!("{:<32} dim {:<5} {}", format!("{:?}", m.model), m.dim, m.description);
            }
            Ok(())
        }
        Some(_) => Err(USAGE.into()),
    };
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

/// Relaunch as a detached process so the terminal is freed and no console window stays open.
fn detach() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("gui");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        cmd.creation_flags(DETACHED_PROCESS);
    }
    cmd.spawn().map(drop).map_err(|e| e.to_string())
}
