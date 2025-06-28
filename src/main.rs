use clap::Parser;
use evdev::{
    AttributeSet, Device, EventType, InputId, KeyCode, MiscCode, PropType, RelativeAxisCode,
    SynchronizationCode, UinputAbsSetup, uinput::VirtualDevice,
};
use figment::providers::Format;

use self::{config::*, logic::*};

mod config;
mod logic;
mod state;
mod utils;

#[derive(clap::Parser)]
struct Cli {
    #[arg(long, short, default_value = "tweakpoint.toml")]
    config: String,
    #[arg(long)]
    dump_config: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive(tracing::level_filters::LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .pretty()
        .init();

    let cli = Cli::parse();

    let config: Config = figment::Figment::new()
        .join(figment::providers::Toml::file(&cli.config))
        .extract()?;

    if cli.dump_config {
        println!("{}", toml::to_string(&config)?);
        return Ok(());
    }

    let mut device = Device::open(&config.device)?;
    device.grab()?;

    tracing::debug!(?device, "Opened and grabbed device");

    let mut dev = VirtualDevice::builder()?
        .name(&config.name)
        .input_id(InputId::new(
            config.bus,
            config.vendor_id,
            config.product_id,
            config.product_version,
        ))
        .with_properties(&AttributeSet::from_iter([PropType::POINTER]))?
        .with_keys(&AttributeSet::from_iter((0..560).map(KeyCode)))?
        .with_relative_axes(&AttributeSet::from_iter(
            (if config.hi_res_enabled {
                0..=12
            } else {
                0..=10
            })
            .map(RelativeAxisCode),
        ))?;
    for (axis, info) in device.get_absinfo()? {
        dev = dev.with_absolute_axis(&UinputAbsSetup::new(axis, info))?;
    }
    if let Some(ff) = device.supported_ff() {
        dev = dev.with_ff(ff)?;
    }
    if let Some(switch) = device.supported_switches() {
        dev = dev.with_switches(switch)?;
    }
    let dev = dev.build()?;

    tracing::debug!(?dev, "Created virtual device");

    let mut udev_stream = dev.into_event_stream()?;

    let socket = if let Some(path) = &config.socket_path {
        if let Ok(true) = tokio::fs::try_exists(path).await {
            tokio::fs::remove_file(path).await?;
        }
        Some(tokio::net::UnixListener::bind(path)?)
    } else {
        None
    };

    let mut controller = Controller::new(config);

    let state_vec_tx = if let Some(socket) = socket {
        let (state_vec_tx, state_vec_rx) = {
            let mut state = vec![];
            controller.state_vec(&mut state);
            tokio::sync::watch::channel(state)
        };
        tokio::spawn(handle_socket(socket, state_vec_rx));
        Some(state_vec_tx)
    } else {
        None
    };

    let mut stream = device.into_event_stream()?;
    let mut buf = vec![];

    tracing::debug!("Starting main loop");
    loop {
        let mut ev = tokio::select! {
            biased;
            _ = controller.next_events(&mut buf) => {
                tracing::trace!(?buf, "Controller emitted events");
                udev_stream.device_mut().emit(&buf)?;
                buf.clear();
                continue;
            }
            evt = udev_stream.next_event() => {
                tracing::trace!(?evt, "Event virtual -> physical");
                stream.device_mut().send_events(&[evt?])?;
                continue;
            }
            evt = stream.next_event() => {
                tracing::trace!(?evt, "Event physical -> virtual");
                evt?
            }
        };

        loop {
            match ev.event_type() {
                EventType::SYNCHRONIZATION if ev.code() == SynchronizationCode::SYN_REPORT.0 => {
                    break;
                }
                EventType::KEY => controller.button(KeyCode(ev.code()), ev.value()),
                EventType::RELATIVE => controller.relative(RelativeAxisCode(ev.code()), ev.value()),
                EventType::MISC if ev.code() == MiscCode::MSC_SCAN.0 => {
                    tracing::trace!(?ev, "Filtered out MSC_SCAN event");
                }
                _ => controller.passthrough(ev),
            }
            if let Some(state_vec_tx) = &state_vec_tx {
                state_vec_tx.send_modify(|x| {
                    x.clear();
                    controller.state_vec(x);
                });
            }
            ev = stream.next_event().await?;
            tracing::trace!(?ev, "Event physical -> virtual");
        }
    }
}

async fn handle_socket(
    socket: tokio::net::UnixListener,
    state_vec_rx: tokio::sync::watch::Receiver<Vec<u8>>,
) {
    loop {
        let Ok((mut conn, _)) = socket.accept().await else {
            tracing::error!("Failed to accept socket connection; bailing, socket is disabled");
            break;
        };
        let mut state_vec_rx = state_vec_rx.clone();
        tokio::spawn(async move {
            use tokio::io::AsyncWriteExt;
            loop {
                let data = state_vec_rx.borrow_and_update().clone();
                if conn.write_all(&data).await.is_err() {
                    break;
                }
                if state_vec_rx.changed().await.is_err() {
                    break;
                };
            }
        });
    }
}
