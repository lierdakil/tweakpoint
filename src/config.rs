use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::PathBuf,
    time::Duration,
};

use evdev::{BusType, EventType, InputEvent, KeyCode, RelativeAxisCode};
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

use crate::{state::State, utils::IteratorExt};

#[derive(Serialize, Deserialize, SmartDefault)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub socket_path: Option<PathBuf>,
    #[default("/dev/input/event0")]
    pub device: PathBuf,
    pub btn_map: BTreeMap<KeyCode, KeyCode>,
    pub meta: MetaConfig,
    #[default("tweakpoint")]
    pub name: String,
    #[default(1)]
    pub vendor_id: u16,
    #[default(1)]
    pub product_id: u16,
    #[default(1)]
    pub product_version: u16,
    #[default(BusType::BUS_USB)]
    pub bus: BusType,
    pub axis_map: AxisMap,
    pub hi_res_enabled: bool,
    #[default(5)]
    pub min_gesture_movement: u32,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default, deny_unknown_fields)]
pub struct AxisMap {
    pub regular: HashMap<RelativeAxisCode, AxisMapDef>,
    pub scroll: HashMap<RelativeAxisCode, AxisMapDef>,
}

impl AxisMap {
    pub fn get(&self, axis: RelativeAxisCode, scroll_active: bool) -> AxisMapDef {
        *scroll_active
            .then(|| self.scroll.get(&axis))
            .flatten()
            .or_else(|| self.regular.get(&axis))
            .inspect(|new| {
                tracing::trace!(old = ?axis, ?new, ?scroll_active, "Mapped axis");
            })
            .unwrap_or(&AxisMapDef { axis, factor: 1.0 })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
#[serde(deny_unknown_fields)]
pub struct AxisMapDef {
    pub axis: RelativeAxisCode,
    #[serde(default = "default_factor")]
    pub factor: f64,
}

fn default_factor() -> f64 {
    1.0
}

#[derive(Serialize, Deserialize, SmartDefault)]
#[serde(default, deny_unknown_fields)]
pub struct MetaConfig {
    #[default(KeyCode::BTN_MIDDLE)]
    pub key: KeyCode,
    #[default(Action::Button(KeyCode::BTN_TASK))]
    pub hold: Action,
    #[default(Action::Button(KeyCode::BTN_TASK))]
    pub r#move: Action,
    #[default([].into())]
    pub chord: BTreeMap<KeyCode, Action>,
    #[default(Duration::from_millis(250))]
    #[serde(with = "humantime_serde")]
    pub hold_time: Duration,
    #[default(Action::ToggleScroll)]
    pub click: Action,
}

pub type Gestures = HashMap<String, Action>;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Action {
    None,
    ToggleScroll,
    ToggleLock(BTreeSet<KeyCode>),
    Button(KeyCode),
    Gesture(Gestures),
}

#[derive(Clone, Copy, Debug)]
#[repr(i32)]
pub enum Direction {
    Up = 0,
    Down = 1,
}

impl Action {
    pub fn run(
        &self,
        state: &mut State,
        dir: Direction,
        ctx: &str,
    ) -> impl IntoIterator<Item = InputEvent> + use<> {
        tracing::debug!(btn_direction = ?dir, action = ?self, %ctx, "Running action");
        match self {
            Action::ToggleScroll if matches!(dir, Direction::Down) => {
                tracing::debug!("ToggleScroll action executing");
                state.scroll.toggle();
                None.left().left()
            }
            Action::Button(key_code) => {
                tracing::debug!(?key_code, "Button action executing");
                Some(InputEvent::new(EventType::KEY.0, key_code.0, dir as i32))
                    .left()
                    .left()
            }
            Action::ToggleLock(lock_btns) if matches!(dir, Direction::Down) => {
                tracing::debug!(?lock_btns, "ToggleLock action executing");
                state.lock.toggle(lock_btns).right().right()
            }
            Action::Gesture(config) => match dir {
                Direction::Down => state.start_gesture().right().left(),
                Direction::Up => state.end_gesture(config).left().right(),
            },
            Action::ToggleScroll | Action::ToggleLock(_) | Action::None => None.left().left(),
        }
    }
}
