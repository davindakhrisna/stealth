use clap::Parser;
use std::process::ExitCode;

use academic::capture::capture_png;
use academic::config::{load_config, set_model};
use academic::error::AcademicError;
use academic::notify::{clear, notify, report_error, simulate};
use academic::router::RouterClient;

#[derive(Parser, Debug)]
#[command(
    name = "academic",
    about = "Screenshot answer helper for Linux Hyprland"
)]
struct Args {
    #[arg(long, short = 'c', help = "Clear dunst notifications")]
    clear: bool,

    #[arg(long, short = 's', help = "Preview notifications")]
    simulate: bool,

    #[arg(long, value_name = "TEXT", help = "Send a notification")]
    notify: Option<String>,

    #[arg(
        long,
        value_name = "MODEL",
        help = "Update ROUTER_MODEL (9Router combo) in .env"
    )]
    set_model: Option<String>,
}

fn run(args: Args) -> Result<(), AcademicError> {
    if args.clear {
        clear()?;
    } else if args.simulate {
        simulate()?;
    } else if let Some(ref text) = args.notify {
        notify(text)?;
    } else if let Some(ref model) = args.set_model {
        let path = set_model(model, None)?;
        println!("ROUTER_MODEL={model} saved to {}", path.display());
    } else {
        let config = load_config()?;
        let client = RouterClient::new(config);
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
