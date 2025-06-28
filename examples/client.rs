use std::{collections::BTreeMap, io::Read};

use evdev::KeyCode;

#[derive(Debug)]
#[repr(u8)]
enum Step {
    Released = 0,
    Locked = 1,
    WillRelease = 2,
}

#[derive(Debug)]
#[expect(dead_code)]
struct State {
    scroll_active: bool,
    buttons: BTreeMap<KeyCode, Step>,
}

impl State {
    fn from_bytes(ptr: &mut &[u8]) -> Self {
        let mut scroll_active = [0u8];
        ptr.read_exact(&mut scroll_active).unwrap();
        let scroll_active = scroll_active[0] != 0;
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
                    0 => Step::Released,
                    1 => Step::Locked,
                    2 => Step::WillRelease,
                    _ => {
                        eprintln!("Unexpected button state for {key_code:?}: {state}");
                        continue;
                    }
                },
            );
        }
        Self {
            scroll_active,
            buttons,
        }
    }
}

fn main() {
    let mut socket =
        std::os::unix::net::UnixStream::connect(std::env::args().nth(1).unwrap()).unwrap();
    let mut size = [0; 4];
    loop {
        socket.read_exact(&mut size).unwrap();
        let mut buf = vec![0; u32::from_le_bytes(size) as usize];
        socket.read_exact(buf.as_mut_slice()).unwrap();
        let ptr = &mut buf.as_slice();
        println!("{:#?}", State::from_bytes(ptr));
    }
}
