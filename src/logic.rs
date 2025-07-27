use evdev::{EventType, InputEvent, KeyCode, RelativeAxisCode};

use crate::{
    config::{Config, Direction},
    state::{ActionType, GestureDir, State},
};

pub struct Controller {
    state: State,
    config: Config,
    synthetic_tx: tokio::sync::mpsc::UnboundedSender<InputEvent>,
    synthetic_rx: tokio::sync::mpsc::UnboundedReceiver<InputEvent>,
    relative_movement: (i32, i32),
}

impl Controller {
    pub fn new(config: Config) -> Self {
        let (synthetic_tx, synthetic_rx) = tokio::sync::mpsc::unbounded_channel();
        Self {
            state: State::default(),
            config,
            synthetic_rx,
            synthetic_tx,
            relative_movement: (0, 0),
        }
    }

    pub fn state_vec(&self, out: &mut Vec<u8>) {
        fn patchback(out: &mut Vec<u8>, action: impl FnOnce(&mut Vec<u8>)) {
            out.extend_from_slice(&[0x00; 4]);
            let pos = out.len();
            action(out);
            let len = (out.len() - pos) as u32;
            out[pos - 4..pos].copy_from_slice(&len.to_le_bytes());
        }
        patchback(out, |out| {
            let scroll_active = self.state.scroll.active;
            out.push(if scroll_active { 0x01 } else { 0x00 });
            patchback(out, |out| {
                for (lock_btn, lock_step) in self.state.lock.state_vec() {
                    out.extend_from_slice(&lock_btn.0.to_le_bytes());
                    out.push(lock_step as u8);
                }
            });
            patchback(out, |out| {
                for dir in self.state.gesture_dir.iter().flatten() {
                    out.push(*dir as u8);
                }
            });
        });
    }

    fn send_events(&self, it: impl IntoIterator<Item = InputEvent>) {
        for evt in it {
            self.synthetic_tx
                .send(evt)
                .expect("Receiver is owned by us, so should be alive");
        }
    }

    pub fn button(&mut self, key_code: KeyCode, value: i32) {
        if key_code == self.config.meta.key {
            match value {
                1 => {
                    // meta key down
                    let this = &mut *self;
                    this.state.meta_down.start_wait(this.config.meta.hold_time);
                }
                0 => {
                    // meta key up
                    let this = &mut *self;
                    let evt = this.state.handle_meta_up(&this.config.meta);
                    this.send_events(evt);
                }
                // meta key ???
                _ => {}
            }
            // don't pass go, don't pass through meta key.
            return;
        }
        if self
            .state
            .meta_down
            .activate_waiting(ActionType::Chord(key_code))
        {
            if let Some(action) = self.config.meta.chord.get(&key_code) {
                tracing::debug!(key = ?key_code, "Activated chord");
                let evts = action.run(
                    &mut self.state,
                    if matches!(value, 1) {
                        Direction::Down
                    } else {
                        Direction::Up
                    },
                    "Chord activated",
                );
                self.send_events(evts);
                // don't pass go, don't emit the chorded button.
                return;
            } else {
                let evts = self.config.meta.hold.run(
                    &mut self.state,
                    Direction::Down,
                    "Hold activated on other button",
                );
                self.send_events(evts);
            }
        }
        let new_key_code = self
            .config
            .btn_map
            .get(&key_code)
            .inspect(|new| {
                tracing::debug!(orig = ?key_code, ?new, "Mapped key press");
            })
            .unwrap_or(&key_code);
        if let Some(new_key_code) = self.state.lock.check(new_key_code, value) {
            self.send_events([InputEvent::new(EventType::KEY.0, new_key_code.0, value)]);
        }
    }

    pub fn start_transaciton(&mut self) {
        self.relative_movement = (0, 0);
    }

    pub fn end_transaciton(&mut self) {
        if let Some(gesture_dir) = &mut self.state.gesture_dir {
            tracing::trace!(?self.relative_movement, "Relative movement");
            if self.relative_movement.0.abs() <= 5 && self.relative_movement.1.abs() <= 5 {
                // movement is insignificant
                return;
            }
            let dir = if self.relative_movement.0.abs() > self.relative_movement.1.abs() {
                // X axis
                if self.relative_movement.0 > 0 {
                    GestureDir::R
                } else {
                    GestureDir::L
                }
            } else {
                // Y axis
                if self.relative_movement.1 > 0 {
                    GestureDir::D
                } else {
                    GestureDir::U
                }
            };
            if gesture_dir.last() != Some(&dir) {
                gesture_dir.push(dir);
            }
        }
    }

    pub fn relative(&mut self, axis: RelativeAxisCode, value: i32) {
        if self.state.meta_down.activate_waiting(ActionType::Move) {
            let evts =
                self.config
                    .meta
                    .r#move
                    .run(&mut self.state, Direction::Down, "Move activated");
            self.send_events(evts);
        }

        match axis {
            RelativeAxisCode::REL_X => self.relative_movement.0 += value,
            RelativeAxisCode::REL_Y => self.relative_movement.1 += value,
            _ => {}
        }

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

    pub async fn next_events(&mut self, buf: &mut Vec<InputEvent>) -> usize {
        loop {
            tokio::select! {
              _ = self.state.meta_down.wait() => {
                  let evts = self.config.meta.hold.run(&mut self.state, Direction::Down, "Hold fired");
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
