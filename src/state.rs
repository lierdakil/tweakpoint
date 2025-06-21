use std::{collections::HashMap, pin::Pin, task::Poll, time::Duration};

use evdev::{InputEvent, RelativeAxisCode};
use futures::FutureExt;

use crate::config::{Action, Direction};

#[derive(Default)]
pub struct State {
    pub meta_down: MetaDown,
    pub scroll: ScrollState,
}

#[derive(Default)]
pub struct MetaDown(MetaDownInner);

#[derive(Default)]
enum MetaDownInner {
    #[default]
    Inactive,
    Waiting(Pin<Box<tokio::time::Sleep>>),
    Active,
}

impl MetaDown {
    pub fn activate_waiting(&mut self) -> bool {
        if matches!(self.0, MetaDownInner::Waiting(_)) {
            tracing::debug!("Force-activating waiting meta_down");
            self.0 = MetaDownInner::Active;
            true
        } else {
            false
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

    fn is_active(&self) -> bool {
        matches!(self.0, MetaDownInner::Active)
    }

    pub fn run(
        &mut self,
        state: &mut ScrollState,
        click: &Action,
        hold: &Action,
    ) -> impl IntoIterator<Item = InputEvent> + use<> {
        tracing::debug!(active = ?self.is_active(), "Running meta_down action");
        let evt = if self.is_active() {
            hold.run(state, Direction::Up)
        } else {
            click.run(state, Direction::Up)
        };
        self.reset();
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
                self.0 = MetaDownInner::Active;
                Poll::Ready(())
            }
            MetaDownInner::Active | MetaDownInner::Inactive => Poll::Pending,
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
