use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    pin::Pin,
    time::Duration,
};

use evdev::{EventType, InputEvent, KeyCode, RelativeAxisCode};

use crate::{
    config::{Action, Direction, Gestures, MetaConfig},
    utils::{EitherIter, IteratorExt},
};

#[derive(Default)]
pub struct State {
    pub meta_down: MetaDown,
    pub scroll: ScrollState,
    pub slow: Option<f64>,
    pub lock: LockState,
    pub gesture_dir: Option<Vec<GestureDir>>,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GestureDir {
    U = b'U',
    D = b'D',
    L = b'L',
    R = b'R',
}

#[derive(Default)]
pub struct LockState {
    btn_states: BTreeMap<KeyCode, LockStep>,
}

#[derive(Clone, Copy, Debug)]
#[repr(u8)]
pub enum LockStep {
    /// Button is ostensibly released both physically and logically.
    Released = b'R',
    /// Button is logically held but physically released
    Locked = b'L',
    /// Button is both logically and physically held, but will be released on
    /// physical release.
    WillRelease = b'W',
}

impl LockStep {
    fn step(&mut self, value: i32) -> bool {
        *self = match *self {
            LockStep::Released if value == 0 => LockStep::Locked,
            LockStep::Locked if value == 1 => LockStep::WillRelease,
            LockStep::WillRelease if value == 0 => LockStep::Released,
            x => x,
        };
        matches!(self, LockStep::Released)
    }
}

impl LockState {
    pub fn state_vec(&self) -> impl Iterator<Item = (KeyCode, LockStep)> {
        self.btn_states.iter().map(|(k, v)| (*k, *v))
    }

    pub fn toggle(
        &mut self,
        btns: &BTreeSet<KeyCode>,
    ) -> impl IntoIterator<Item = InputEvent> + use<> {
        if self.btn_states.is_empty() {
            tracing::debug!(state = "on", ?btns, "Toggle lock state");
            self.btn_states = btns.iter().map(|x| (*x, LockStep::Released)).collect();
            vec![]
        } else {
            tracing::debug!(state = "off", ?btns, "Toggle lock state");
            // to keep sane, release locked buttons
            let res = self
                .btn_states
                .iter()
                .filter_map(|(key, step)| {
                    if matches!(step, LockStep::Locked) {
                        tracing::debug!(?key, "Releasing locked key");
                        Some(InputEvent::new(EventType::KEY.0, key.0, 0))
                    } else {
                        None
                    }
                })
                .collect();
            self.btn_states.clear();
            res
        }
    }

    pub fn check(&mut self, button: &KeyCode, value: i32) -> Option<KeyCode> {
        if let Some(entry) = self.btn_states.get_mut(button) {
            // "lock" just filters out consecutive {0, 1} sequences.
            if !entry.step(value) {
                tracing::debug!(?entry, ?button, "Locking button");
                return None;
            }
        }
        Some(*button)
    }
}

#[derive(Default)]
pub struct MetaDown {
    inner: MetaDownInner,
}

#[derive(Default)]
enum MetaDownInner {
    #[default]
    Inactive,
    Waiting(Pin<Box<tokio::time::Sleep>>),
    Active(ActionType),
}

#[derive(Clone, Copy, Debug)]
pub enum ActionType {
    Hold,
    Move,
    Chord(KeyCode),
}

#[derive(Clone, Copy, Debug)]
pub enum ActionTypeExt {
    Click,
    Other(ActionType),
}

impl MetaDown {
    pub fn activate_waiting(&mut self, typ: ActionType) -> bool {
        if matches!(self.inner, MetaDownInner::Waiting(_)) {
            tracing::debug!(?typ, "Force-activating waiting meta_down");
            self.inner = MetaDownInner::Active(typ);
            true
        } else {
            match (typ, &self.inner) {
                (ActionType::Chord(k1), MetaDownInner::Active(ActionType::Chord(k2)))
                    if &k1 == k2 =>
                {
                    tracing::debug!(key = ?k1, "Detected chord release event");
                    true
                }
                _ => false,
            }
        }
    }

    pub fn start_wait(&mut self, timeout: Duration) {
        tracing::debug!(?timeout, "Started meta_down timer");
        self.inner = MetaDownInner::Waiting(Box::pin(tokio::time::sleep(timeout)));
    }

    fn reset(&mut self) {
        tracing::debug!("Reset meta_down to inactive");
        self.inner = MetaDownInner::Inactive;
    }

    fn action_type(&self) -> ActionTypeExt {
        if let MetaDownInner::Active(typ) = &self.inner {
            ActionTypeExt::Other(*typ)
        } else {
            ActionTypeExt::Click
        }
    }

    pub async fn wait(&mut self) {
        // we hold a mut reference, mening nobody else does. Ergo, it can't
        // change from under us, ergo we can return pending() in the
        // else-branch: the future would have to be canned before inner can
        // change.
        match &mut self.inner {
            MetaDownInner::Waiting(pin) => {
                pin.await;
                tracing::debug!("meta_down timeout triggered");
                self.inner = MetaDownInner::Active(ActionType::Hold);
            }
            MetaDownInner::Active(_) | MetaDownInner::Inactive => std::future::pending().await,
        }
    }
}

impl State {
    pub fn handle_meta_up(
        &mut self,
        config: &MetaConfig,
    ) -> impl IntoIterator<Item = InputEvent> + use<> {
        tracing::debug!(action_type = ?self.meta_down.action_type(), "Running meta_up action");
        let evt = match self.meta_down.action_type() {
            ActionTypeExt::Click => {
                // the only "instant" action, run down then immediately up.
                EitherIter::right(
                    config
                        .click
                        .run(self, Direction::Down, "Click on meta_up")
                        .into_iter()
                        .chain(config.click.run(self, Direction::Up, "Click on meta_up")),
                )
            }
            ActionTypeExt::Other(ActionType::Hold) => config
                .hold
                .run(self, Direction::Up, "Hold on meta_up")
                .into(),
            ActionTypeExt::Other(ActionType::Move) => config
                .r#move
                .run(self, Direction::Up, "Move on meta_up")
                .into(),
            // chord is handled on chorded button press/release, so we nothing to do here.
            ActionTypeExt::Other(ActionType::Chord(_)) => Action::None
                .run(self, Direction::Up, "Chord on meta_up")
                .into(),
        };
        self.meta_down.reset();
        evt
    }

    pub fn start_gesture(&mut self) -> impl IntoIterator<Item = InputEvent> + use<> {
        self.gesture_dir = Some(vec![]);
        std::iter::empty()
    }

    pub fn end_gesture(
        &mut self,
        config: &Gestures,
    ) -> impl IntoIterator<Item = InputEvent> + use<> {
        // TODO: too much cloning happening here
        let Some(gesture_dir) = self.gesture_dir.take() else {
            return std::iter::empty().left();
        };
        let key = gesture_dir
            .iter()
            .map(|x| format!("{x:?}"))
            .collect::<Vec<_>>()
            .join("");
        tracing::debug!(?key, "Gesture activated");
        if let Some(action) = config.get(&key) {
            tracing::debug!(?action, "Gesture action");
            action
                .run(self, Direction::Down, "Gesture down")
                .into_iter()
                .chain(action.run(self, Direction::Up, "Gesture up"))
                .collect::<Vec<_>>()
                .into_iter()
                .right()
        } else {
            std::iter::empty().left()
        }
    }
}

#[derive(Default)]
pub struct ScrollState {
    pub active: bool,
    pub axes: HashMap<RelativeAxisCode, f64>,
}

impl ScrollState {
    pub fn toggle(&mut self) {
        self.active = !self.active;
        tracing::debug!(active = ?self.active, "Scroll state toggled");
        if self.active {
            self.axes.clear();
        }
    }

    pub fn scroll(&mut self, axis: RelativeAxisCode, value: i32, factor: f64) -> i32 {
        let axis_buf = self.axes.entry(axis).or_insert(0.0);
        *axis_buf += f64::from(value) * factor;
        let trunc = *axis_buf as i32;
        *axis_buf -= f64::from(trunc);
        trunc
    }
}
