use clap::Parser;
use std::process::ExitCode;

use stealth::capture::capture_png;
use stealth::config::{load_config, set_effort, set_model};
use stealth::engine::EngineClient;
use stealth::error::StealthError;
use stealth::notify::{clear, notify, report_error, simulate};

#[derive(Parser, Debug)]
#[command(
    name = "stealth",
    about = "Screenshot answer helper for Linux Hyprland"
)]
struct Args {
    #[arg(long, short = 'c', help = "Clear dunst notifications")]
    clear: bool,

    #[arg(long, short = 's', help = "Preview notifications")]
    simulate: bool,

    #[arg(long, value_name = "TEXT", help = "Send a notification")]
    notify: Option<String>,

    #[arg(long, value_name = "MODEL", help = "Update AGY_MODEL in .env")]
    set_model: Option<String>,

    #[arg(
        long,
        value_name = "EFFORT",
        help = "Update AGY_EFFORT (low|medium|high|xhigh|max) in .env"
    )]
    set_effort: Option<String>,
}

fn run(args: Args) -> Result<(), StealthError> {
    if args.clear {
        clear()?;
    } else if args.simulate {
        simulate()?;
    } else if let Some(ref text) = args.notify {
        notify(text)?;
    } else if let Some(ref model) = args.set_model {
        let path = set_model(model, None)?;
        println!("AGY_MODEL={model} saved to {}", path.display());
    } else if let Some(ref effort) = args.set_effort {
        let path = set_effort(effort, None)?;
        println!("AGY_EFFORT={effort} saved to {}", path.display());
    } else {
        let config = load_config()?;
        let client = EngineClient::new(config);
        let image = capture_png()?;
        let answer = client.generate(&image)?;
        notify(&answer)?;
    }
    Ok(())
}

fn main() -> ExitCode {
    let args = Args::parse();
    if let Err(err) = run(args) {
        report_error(&err.to_string());
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
