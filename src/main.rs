extern crate uinput;
extern crate uinput_sys;

use std::{
    fs::File,
    io::{self},
    os::unix::prelude::*,
};
use std::env;
use std::fs::OpenOptions;
use std::io::{Error, Read};
use std::mem;
use std::os::raw::c_int;

use ioctl_rs;
use uinput_sys::*;
use uinput_sys::input_event;

fn grab_keyboard(fd: c_int) -> Result<(), Error> {
    const EVIOCGRAB: u32 = 1074021776;
    match unsafe { ioctl_rs::ioctl(fd, EVIOCGRAB, 1) } {
        0 => Ok(()),
        _ => Err(Error::last_os_error()),
    }
}


fn map(ev: &mut input_event, keys: &mut [i32], layer: i32) {
    let code = ev.code as usize;

    if ev.value == 1 {
        keys[code] = layer;
    }
    println!("{}", layer);
    if keys[code] == -1 {
        return;
    }

    ev.code = match (code as i32, keys[code]) {
        (KEY_RIGHTCTRL, 0) => KEY_RIGHTMETA,
        (KEY_LEFTALT, 0) => KEY_LEFTCTRL,
        (KEY_CAPSLOCK, 0) => KEY_LEFTALT,
        (KEY_LEFTCTRL, 0) => KEY_RIGHTALT,
        (KEY_SYSRQ, 0) => KEY_RIGHTCTRL,

        (KEY_L, 1) => KEY_RIGHT,
        (KEY_I, 1) => KEY_UP,
        (KEY_J, 1) => KEY_LEFT,
        (KEY_K, 1) => KEY_DOWN,

        (KEY_U, 1) => KEY_PAGEUP,
        (KEY_O, 1) => KEY_PAGEDOWN,
        (KEY_SEMICOLON, 1) => KEY_END,
        (KEY_P, 1) => KEY_HOME,
        (KEY_N, 1) => KEY_INSERT,

        (KEY_COMMA, 1) => KEY_COMPOSE,

        (KEY_1, 1) => KEY_F1,
        (KEY_2, 1) => KEY_F2,
        (KEY_3, 1) => KEY_F3,
        (KEY_4, 1) => KEY_F4,
        (KEY_5, 1) => KEY_F5,
        (KEY_6, 1) => KEY_F6,
        (KEY_7, 1) => KEY_F7,
        (KEY_8, 1) => KEY_F8,
        (KEY_9, 1) => KEY_F9,
        (KEY_MINUS, 1) => KEY_F11,
        (KEY_EQUAL, 1) => KEY_F12,

        (KEY_HOME, 1) => KEY_BRIGHTNESSUP,
        (KEY_F12, 1) => KEY_BRIGHTNESSDOWN,
        (KEY_F11, 1) => KEY_VOLUMEUP,
        (KEY_F10, 1) => KEY_VOLUMEDOWN,
        (KEY_F9, 1) => KEY_MUTE,
        (KEY_RIGHTSHIFT, 1) => KEY_CAPSLOCK,
        (KEY_RIGHTBRACE, 1) => KEY_NEXTSONG,
        (KEY_LEFTBRACE, 1) => KEY_PREVIOUSSONG,
        (KEY_BACKSLASH, 1) => KEY_PLAYPAUSE,

        (KEY_H, 1) => KEY_MENU,
        (KEY_Y, 1) => KEY_PROG4,

        (KEY_M, 1) => KEY_BACKSPACE,
        (KEY_DOT, 1) => KEY_DELETE,

        (KEY_LEFTALT, 1) => KEY_LEFTCTRL,
        (KEY_CAPSLOCK, 1) => KEY_LEFTALT,
        (KEY_LEFTCTRL, 1) => KEY_RIGHTALT,
        (KEY_SYSRQ, 1) => KEY_RIGHTCTRL,
        (left, _) => left,
    } as u16;
}


fn listen() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let mut file: File = OpenOptions::new()
        .read(true)
        .write(false)
        .open(args[1].clone())?;
    let fd = file.as_raw_fd();

    grab_keyboard(fd)?;
    const MAX_EVS: usize = 1024;
    const BUF_SIZE: usize = mem::size_of::<input_event>() * MAX_EVS;
    let mut raw_buffer = [0u8; BUF_SIZE];
    let mut builder = uinput::default().unwrap()
        .name("test").unwrap()
        .event(uinput::event::Keyboard::All).unwrap()
        .event(uinput::event::Controller::All).unwrap();

    for item in uinput::event::relative::Position::iter_variants() {
        builder = builder.event(item).unwrap();
    }

    for item in uinput::event::relative::Wheel::iter_variants() {
        builder = builder.event(item).unwrap();
    }

    let mut device = builder.create().unwrap();


    let mut current_layer = 0;
    let mut layers = [-1i32; 1024];
    loop {
        let events_count = file.read(&mut raw_buffer)? / mem::size_of::<input_event>();
        let mut events = unsafe {
            mem::transmute::<[u8; BUF_SIZE], [input_event; MAX_EVS]>(raw_buffer)
        };
        println!("*********");
        for i in 0..events_count {
            println!("{} {} {}", events[i].kind, events[i].code, events[i].value);
        }
        for i in 0..events_count {
            let mut skip = false;
            if events[i].kind == EV_KEY as u16 {
                if events[i].code as i32 == KEY_RIGHTALT {
                    skip = true;
                    if events[i].value == 0 {
                        current_layer = 0;
                    } else {
                        current_layer = 1;
                    }
                } else {
                    println!("before{}", events[i].code);
                    map(&mut events[i], &mut layers, current_layer);
                    println!("after{}", events[i].code);
                }
            }
            if !skip {
                device.write(events[i].kind as i32, events[i].code as i32, events[i].value).unwrap();
            }
        }
    }
}

fn main() -> Result<(), Error> {
    listen()
}