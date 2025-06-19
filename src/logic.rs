use evdev::{
    AbsoluteAxisCode, AttributeSet, Device, EventType, InputEvent, InputId, KeyCode, PropType,
    RelativeAxisCode, UinputAbsSetup, uinput::VirtualDevice,
};

use crate::{
    config::Config,
    state::{MetaDown, State},
};

pub struct Controller {
    state: State,
    dev: VirtualDevice,
    config: Config,
}

impl Controller {
    pub fn new(config: Config, device: &Device) -> anyhow::Result<Self> {
        let mut dev = VirtualDevice::builder()?
            .name(&config.name)
            .input_id(InputId::new(
                config.bus,
                config.vendor_id,
                config.product_id,
                config.product_version,
            ))
            .with_properties(&AttributeSet::from_iter([PropType::POINTER]))?;

        for (axis, info) in device.get_absinfo()? {
            dev = dev.with_absolute_axis(&UinputAbsSetup::new(axis, info))?;
        }
        dev = dev.with_keys(&AttributeSet::from_iter((0..560).map(KeyCode)))?;
        dev = dev.with_relative_axes(&AttributeSet::from_iter(
            (if config.hi_res_enabled {
                0..=12
            } else {
                0..=10
            })
            .map(RelativeAxisCode),
        ))?;
        if let Some(ff) = device.supported_ff() {
            dev = dev.with_ff(ff)?;
        }
        if let Some(swtch) = device.supported_switches() {
            dev = dev.with_switches(swtch)?;
        }
        let dev = dev.build()?;
        Ok(Self {
            state: State::default(),
            dev,
            config,
        })
    }

    pub async fn run_init(&self) -> anyhow::Result<()> {
        if let Some(init) = &self.config.init {
            tokio::process::Command::new("/usr/bin/env")
                .args(["sh", "-c", init])
                .spawn()?
                .wait()
                .await?;
        }
        Ok(())
    }

    pub async fn handle_meta_down(&mut self) -> anyhow::Result<()> {
        (&mut self.state.meta_down).await;
        self.config
            .meta
            .hold
            .run(&mut self.state, &mut self.dev, true)?;
        Ok(())
    }

    pub fn meta_down(&mut self) {
        self.state.meta_down =
            MetaDown::Waiting(Box::pin(tokio::time::sleep(self.config.meta.hold_time)));
    }

    pub fn meta_up(&mut self) -> anyhow::Result<()> {
        match self.state.meta_down {
            MetaDown::Waiting(_) | MetaDown::Inactive => {
                self.config
                    .meta
                    .click
                    .run(&mut self.state, &mut self.dev, false)?;
            }
            MetaDown::Active => {
                self.config
                    .meta
                    .hold
                    .run(&mut self.state, &mut self.dev, false)?;
            }
        }
        self.state.meta_down = MetaDown::Inactive;
        Ok(())
    }

    fn handle_pre_input(&mut self) -> anyhow::Result<()> {
        if self.state.meta_down.activate_waiting() {
            self.config
                .meta
                .hold
                .run(&mut self.state, &mut self.dev, true)?;
        }
        Ok(())
    }

    pub fn button(&mut self, key_code: KeyCode, value: i32) -> anyhow::Result<()> {
        if key_code == self.config.meta.key {
            match value {
                1 => {
                    self.meta_down();
                    return Ok(());
                }
                0 => return self.meta_up(),
                _ => {}
            }
        }
        self.handle_pre_input()?;
        let new_key_code = self.config.btn_map.get(&key_code).unwrap_or(&key_code);
        self.dev
            .emit(&[InputEvent::new(EventType::KEY.0, new_key_code.0, value)])?;
        Ok(())
    }

    pub fn relative(&mut self, axis: RelativeAxisCode, value: i32) -> anyhow::Result<()> {
        self.handle_pre_input()?;
        let new_axis = self.config.axis_map.get(axis, self.state.scroll.active);
        let new_value = self
            .state
            .scroll
            .scroll(new_axis.axis, value, new_axis.factor);
        self.dev.emit(&[InputEvent::new(
            EventType::RELATIVE.0,
            new_axis.axis.0,
            new_value,
        )])?;

        Ok(())
    }

    pub fn absolute(&mut self, axis: AbsoluteAxisCode, value: i32) -> anyhow::Result<()> {
        self.handle_pre_input()?;
        self.passthrough(InputEvent::new(EventType::ABSOLUTE.0, axis.0, value))?;
        Ok(())
    }

    pub fn passthrough(&mut self, ev: InputEvent) -> anyhow::Result<()> {
        self.dev.emit(&[ev])?;
        Ok(())
    }
}
