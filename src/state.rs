use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    pin::Pin,
    task::Poll,
    time::Duration,
};

use evdev::{EventType, InputEvent, KeyCode, RelativeAxisCode};
use futures::FutureExt;

use crate::{
    config::{Action, Direction, MetaConfig},
    utils::EitherIter,
};

#[derive(Default)]
pub struct State {
    pub meta_down: MetaDown,
    pub scroll: ScrollState,
    pub lock: LockState,
}

#[derive(Default)]
pub struct LockState {
    btn_states: BTreeMap<KeyCode, LockStep>,
}

#[derive(Clone, Copy, Debug)]
enum LockStep {
    /// Button is ostensibly released both physically and logically.
    Released,
    /// Button is logically held but physically released
    Locked,
    /// Button is both logically and physically held, but will be released on
    /// physical release.
    WillRelease,
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
pub struct MetaDown(MetaDownInner);

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
        if matches!(self.0, MetaDownInner::Waiting(_)) {
            tracing::debug!(?typ, "Force-activating waiting meta_down");
            self.0 = MetaDownInner::Active(typ);
            true
        } else {
            match (typ, &self.0) {
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
        self.0 = MetaDownInner::Waiting(Box::pin(tokio::time::sleep(timeout)));
    }

    fn reset(&mut self) {
        tracing::debug!("Reset meta_down to inactive");
        self.0 = MetaDownInner::Inactive;
    }

    fn action_type(&self) -> ActionTypeExt {
        if let MetaDownInner::Active(typ) = &self.0 {
            ActionTypeExt::Other(*typ)
        } else {
            ActionTypeExt::Click
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
}

impl Future for &mut MetaDown {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        match &mut self.0 {
            MetaDownInner::Waiting(pin) => {
                futures::ready!(pin.poll_unpin(cx));
                tracing::debug!("meta_down timeout triggered");
                self.0 = MetaDownInner::Active(ActionType::Hold);
                Poll::Ready(())
            }
            MetaDownInner::Active(_) | MetaDownInner::Inactive => Poll::Pending,
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
