//! Waybar sample client

use std::{collections::BTreeMap, io::Read};

use evdev::KeyCode;

#[derive(Debug)]
enum Step {
    Released,
    Locked,
    WillRelease,
}

#[derive(Debug)]
pub enum GestureDir {
    U,
    D,
    L,
    R,
}

#[derive(Debug)]
struct State {
    scroll_active: bool,
    buttons: BTreeMap<KeyCode, Step>,
    gesture: Vec<GestureDir>,
}

impl State {
    fn from_bytes(ptr: &mut &[u8]) -> Self {
        fn with_len<R>(ptr: &mut &[u8], action: impl FnOnce(&mut &[u8]) -> R) -> R {
            let mut len = [0u8; 4];
            ptr.read_exact(&mut len).unwrap();
            let len = u32::from_le_bytes(len);
            let (mut cur, rest) = ptr.split_at(len as usize);
            *ptr = rest;
            action(&mut cur)
        }
        let mut scroll_active = [0u8];
        ptr.read_exact(&mut scroll_active).unwrap();
        let scroll_active = scroll_active[0] != 0;
        let buttons = with_len(ptr, |ptr| {
            let mut buttons = BTreeMap::new();
            while !ptr.is_empty() {
                let mut key_code = [0u8; 2];
                ptr.read_exact(&mut key_code).unwrap();
                let key_code = evdev::KeyCode::new(u16::from_le_bytes(key_code));
                let mut state = [0u8];
                ptr.read_exact(&mut state).unwrap();
                let state = state[0];
                buttons.insert(
                    key_code,
                    match state {
                        b'R' => Step::Released,
                        b'L' => Step::Locked,
                        b'W' => Step::WillRelease,
                        _ => {
                            eprintln!("Unexpected button state for {key_code:?}: {state}");
                            continue;
                        }
                    },
                );
            }
            buttons
        });
        let gesture = with_len(ptr, |ptr| {
            let mut gesture = Vec::new();
            while !ptr.is_empty() {
                let mut byte = [0u8];
                ptr.read_exact(&mut byte).unwrap();
                let byte = byte[0];
                gesture.push(match byte {
                    b'U' => GestureDir::U,
                    b'D' => GestureDir::D,
                    b'L' => GestureDir::L,
                    b'R' => GestureDir::R,
                    _ => {
                        eprintln!("Unexpected gesture direction {byte}");
                        continue;
                    }
                });
            }
            gesture
        });
        Self {
            scroll_active,
            buttons,
            gesture,
        }
    }
}

fn main() {
    let mut socket = std::os::unix::net::UnixStream::connect(
        std::env::args()
            .nth(1)
            .unwrap_or("/tmp/tweakpoint.sock".to_owned()),
    )
    .unwrap();
    let mut size = [0; 4];
    loop {
        socket.read_exact(&mut size).unwrap();
        let mut buf = vec![0; u32::from_le_bytes(size) as usize];
        socket.read_exact(buf.as_mut_slice()).unwrap();
        let ptr = &mut buf.as_slice();
        let state = State::from_bytes(ptr);
        let scroll_lock = if state.scroll_active { "󰆾" } else { "" };
        let btn_lock = if !state.buttons.is_empty() {
            if state.buttons.values().any(|x| matches!(x, Step::Locked)) {
                "󱕐"
            } else {
                "󱕑"
            }
        } else {
            ""
        };
        let gesture = state
            .gesture
            .iter()
            .map(|x| match x {
                GestureDir::U => "↑",
                GestureDir::D => "↓",
                GestureDir::L => "←",
                GestureDir::R => "→",
            })
            .collect::<String>();
        let text = format!("{scroll_lock}{btn_lock}{gesture}");
        println!(r#"{{ "text": "{text}", "class": "tweakpoint" }}"#,);
    }
}
