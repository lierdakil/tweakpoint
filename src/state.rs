use std::{collections::HashMap, pin::Pin, task::Poll};

use evdev::RelativeAxisCode;
use futures::FutureExt;

#[derive(Default)]
pub struct State {
    pub meta_down: MetaDown,
    pub scroll: ScrollState,
}

#[derive(Default)]
pub enum MetaDown {
    #[default]
    Inactive,
    Waiting(Pin<Box<tokio::time::Sleep>>),
    Active,
}

impl MetaDown {
    pub fn activate_waiting(&mut self) -> bool {
        if matches!(self, MetaDown::Waiting(_)) {
            *self = MetaDown::Active;
            true
        } else {
            false
        }
    }
}

impl Future for &mut MetaDown {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        match &mut *self {
            MetaDown::Waiting(pin) => {
                futures::ready!(pin.poll_unpin(cx));
                **self = MetaDown::Active;
                Poll::Ready(())
            }
            MetaDown::Active | MetaDown::Inactive => Poll::Pending,
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
