use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    time::Duration,
};

use evdev::{BusType, EventType, InputEvent, KeyCode, RelativeAxisCode};
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

use crate::state::ScrollState;

#[derive(Serialize, Deserialize, SmartDefault)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
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
                tracing::debug!(old = ?axis, ?new, ?scroll_active, "Mapped axis");
            })
            .unwrap_or(&AxisMapDef { axis, factor: 1.0 })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
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
    #[default(Duration::from_millis(250))]
    #[serde(with = "humantime_serde")]
    pub hold_time: Duration,
    #[default(Action::ToggleScroll)]
    pub click: Action,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Action {
    ToggleScroll,
    Button(KeyCode),
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
        state: &mut ScrollState,
        dir: Direction,
    ) -> impl IntoIterator<Item = InputEvent> + use<> {
        tracing::debug!(btn_direction = ?dir, action = ?self, "Running action");
        match self {
            Action::ToggleScroll => {
                state.toggle();
                None
            }
            Action::Button(key_code) => {
                Some(InputEvent::new(EventType::KEY.0, key_code.0, dir as i32))
            }
        }
    }
}
