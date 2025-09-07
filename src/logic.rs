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

    pub fn start_transaction(&mut self) -> Transaction<'_> {
        Transaction::new(self)
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
}

pub struct Transaction<'a> {
    ctl: &'a mut Controller,
    relative_movement: (i32, i32),
}

impl<'a> Transaction<'a> {
    fn new(ctl: &'a mut Controller) -> Self {
        Self {
            ctl,
            relative_movement: (0, 0),
        }
    }

    pub fn button(&mut self, key_code: KeyCode, value: i32) {
        let ctl = &mut self.ctl;
        if key_code == ctl.config.meta.key {
            match value {
                1 => {
                    // meta key down
                    ctl.state.meta_down.start_wait(ctl.config.meta.hold_time);
                }
                0 => {
                    // meta key up
                    let evt = ctl.state.handle_meta_up(&ctl.config.meta);
                    ctl.send_events(evt);
                }
                // meta key ???
                _ => {}
            }
            // don't pass go, don't pass through meta key.
            return;
        }
        let mapped_key = ctl
            .config
            .btn_map
            .get(&key_code)
            .inspect(|new| {
                tracing::debug!(orig = ?key_code, ?new, "Mapped key press");
            })
            .unwrap_or(&key_code);
        if let Some(action) = ctl.config.meta.chord.get(mapped_key) {
            if ctl
                .state
                .meta_down
                .activate_waiting(ActionType::Chord(*mapped_key))
            {
                tracing::debug!(key = ?mapped_key, "Activated chord");
                let evts = action.run(
                    &mut ctl.state,
                    if matches!(value, 1) {
                        Direction::Down
                    } else {
                        Direction::Up
                    },
                    "Chord activated",
                );
                ctl.send_events(evts);
                // don't pass go, don't emit the chorded button.
                return;
            }
        } else if ctl.state.meta_down.activate_waiting(ActionType::Hold) {
            let evts = ctl.config.meta.hold.run(
                &mut ctl.state,
                Direction::Down,
                "Hold activated on other button",
            );
            ctl.send_events(evts);
        }

        if let Some(mapped_key) = ctl.state.lock.check(mapped_key, value) {
            ctl.send_events([InputEvent::new(EventType::KEY.0, mapped_key.0, value)]);
        }
    }

    pub fn relative(&mut self, axis: RelativeAxisCode, value: i32) {
        let ctl = &mut self.ctl;
        if ctl.state.meta_down.activate_waiting(ActionType::Move) {
            let evts =
                ctl.config
                    .meta
                    .r#move
                    .run(&mut ctl.state, Direction::Down, "Move activated");
            ctl.send_events(evts);
        }

        // this is a little weird: use remapped axes for gestures, but ignore
        // scroll toggle and axis factors. There may be a more natural option
        // hiding here somewhere but I don't see it.
        match ctl.config.axis_map.get(axis, false).axis {
            RelativeAxisCode::REL_X => self.relative_movement.0 += value,
            RelativeAxisCode::REL_Y => self.relative_movement.1 += value,
            _ => {}
        }

        let new_axis = ctl.config.axis_map.get(axis, ctl.state.scroll.active);
        let new_value = ctl
            .state
            .scroll
            .scroll(new_axis.axis, value, new_axis.factor);

        if ctl.config.move_during_gesture || ctl.state.gesture_dir.is_none() {
            ctl.send_events([InputEvent::new(
                EventType::RELATIVE.0,
                new_axis.axis.0,
                new_value,
            )]);
        }
    }

    pub fn passthrough(&self, ev: InputEvent) {
        self.ctl.send_events([ev]);
    }
}

impl Drop for Transaction<'_> {
    fn drop(&mut self) {
        let Self {
            ctl,
            relative_movement,
        } = self;
        if relative_movement.0.unsigned_abs() <= ctl.config.min_gesture_movement
            && relative_movement.1.unsigned_abs() <= ctl.config.min_gesture_movement
        {
            // movement is insignificant
            return;
        }
        if let Some(gesture_dir) = &mut ctl.state.gesture_dir {
            tracing::trace!(?relative_movement, "Relative movement");
            let dir = if relative_movement.0.abs() > relative_movement.1.abs() {
                // X axis
                if relative_movement.0 > 0 {
                    GestureDir::R
                } else {
                    GestureDir::L
                }
            } else {
                // Y axis
                if relative_movement.1 > 0 {
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
}
