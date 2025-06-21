use evdev::{AbsoluteAxisCode, EventType, InputEvent, KeyCode, RelativeAxisCode};

use crate::{
    config::{Config, Direction},
    state::State,
};

pub struct Controller {
    state: State,
    config: Config,
    synthetic_tx: tokio::sync::mpsc::UnboundedSender<InputEvent>,
    synthetic_rx: tokio::sync::mpsc::UnboundedReceiver<InputEvent>,
}

impl Controller {
    pub fn new(config: Config) -> Self {
        let (synthetic_tx, synthetic_rx) = tokio::sync::mpsc::unbounded_channel();
        Self {
            state: State::default(),
            config,
            synthetic_rx,
            synthetic_tx,
        }
    }

    fn send_events(&self, it: impl IntoIterator<Item = InputEvent>) {
        for evt in it {
            self.synthetic_tx
                .send(evt)
                .expect("Receiver is owned by us, so should be alive");
        }
    }

    pub fn meta_down(&mut self) {
        self.state.meta_down.start_wait(self.config.meta.hold_time);
    }

    pub fn meta_up(&mut self) {
        let evt = self.state.meta_down.run(
            &mut self.state.scroll,
            &self.config.meta.click,
            &self.config.meta.hold,
        );
        self.send_events(evt);
    }

    fn handle_pre_input(&mut self) {
        if self.state.meta_down.activate_waiting() {
            let evts = self
                .config
                .meta
                .hold
                .run(&mut self.state.scroll, Direction::Down);
            self.send_events(evts);
        }
    }

    pub fn button(&mut self, key_code: KeyCode, value: i32) {
        if key_code == self.config.meta.key {
            match value {
                1 => return self.meta_down(),
                0 => return self.meta_up(),
                _ => {}
            }
        }
        self.handle_pre_input();
        let new_key_code = self
            .config
            .btn_map
            .get(&key_code)
            .inspect(|new| {
                tracing::debug!(orig = ?key_code, ?new, "Mapped key press");
            })
            .unwrap_or(&key_code);
        self.send_events([InputEvent::new(EventType::KEY.0, new_key_code.0, value)]);
    }

    pub fn relative(&mut self, axis: RelativeAxisCode, value: i32) {
        self.handle_pre_input();
        let new_axis = self.config.axis_map.get(axis, self.state.scroll.active);
        let new_value = self
            .state
            .scroll
            .scroll(new_axis.axis, value, new_axis.factor);
        self.send_events([InputEvent::new(
            EventType::RELATIVE.0,
            new_axis.axis.0,
            new_value,
        )]);
    }

    pub fn absolute(&mut self, axis: AbsoluteAxisCode, value: i32) {
        self.handle_pre_input();
        self.send_events([InputEvent::new(EventType::ABSOLUTE.0, axis.0, value)]);
    }

    pub async fn next_events(&mut self, buf: &mut Vec<InputEvent>) -> usize {
        loop {
            tokio::select! {
              _ = &mut self.state.meta_down => {
                  let evts = self.config.meta.hold.run(&mut self.state.scroll, Direction::Down);
                  self.send_events(evts);
              },
              n = self.synthetic_rx.recv_many(buf, usize::MAX) => {
                  return n;
              }
            }
        }
    }

    pub fn passthrough(&self, ev: InputEvent) {
        self.send_events([ev]);
    }
}
