use clap::Parser;
use evdev::{Device, EventType, KeyCode, RelativeAxisCode, SynchronizationCode};
use figment::providers::Format;

use self::{config::*, logic::*};

mod config;
mod logic;
mod state;

#[derive(clap::Parser)]
struct Cli {
    #[arg(long, short, default_value = "tweakpoint.toml")]
    config: String,
    #[arg(long)]
    dump_config: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let config: Config = figment::Figment::new()
        .join(figment::providers::Toml::file(&cli.config))
        .extract()?;

    if cli.dump_config {
        println!("{}", toml::to_string(&config)?);
        return Ok(());
    }

    let mut device = Device::open(&config.device)?;
    device.set_nonblocking(true)?;
    device.grab()?;

    let mut controller = Controller::new(config, &device)?;

    controller.run_init().await?;

    let mut stream = device.into_event_stream()?;

    loop {
        let next = tokio::select! {
            next = stream.next_event() => next,
            res = controller.handle_meta_down() => {
                res?;
                continue;
            }
        };

        let ev = match next {
            Ok(x) => x,
            Err(e) => {
                eprintln!("Error: {e}");
                continue;
            }
        };

        match ev.event_type() {
            // SYN_REPORT is already sent by emit, so filter those out.
            EventType::SYNCHRONIZATION if ev.code() == SynchronizationCode::SYN_REPORT.0 => {}
            EventType::KEY => controller.button(KeyCode(ev.code()), ev.value())?,
            EventType::RELATIVE => {
                controller.relative(RelativeAxisCode(ev.code()), ev.value())?;
            }
            EventType::ABSOLUTE => {
                controller.absolute(evdev::AbsoluteAxisCode(ev.code()), ev.value())?;
            }
            _ => controller.passthrough(ev)?,
        }
    }
}
