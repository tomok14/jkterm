mod config;
mod terminal;
mod parser;
mod pty;
mod renderer;

use std::sync::Arc;
use std::thread;

use winit::event::{ElementState, Event, KeyEvent, WindowEvent};
use winit::event_loop::EventLoop;
use winit::keyboard::{Key, NamedKey};
use winit::window::Window;

use crate::config::Config;
use crate::terminal::Terminal;
use crate::parser::Parser as VteParser;

enum AppEvent {
    PtyOutput(Vec<u8>),
    PtyExited,
}

fn main() {
    env_logger::init();
    let config = Config::default();

    let event_loop = EventLoop::<AppEvent>::with_user_event()
        .build()
        .expect("build event loop");

    let window_attrs = Window::default_attributes()
        .with_title("jkterm")
        .with_inner_size(winit::dpi::LogicalSize::new(
            80.0 * 9.0 + 16.0,
            24.0 * 17.0 + 16.0,
        ));
    let window = Arc::new(event_loop.create_window(window_attrs).unwrap());

    let mut renderer = pollster::block_on(renderer::Renderer::new(&window, &config));
    let (cw, ch) = renderer.cell_size();
    let (cols, rows) = renderer.terminal_size();
    log::info!("terminal size: {cols}x{rows}");

    let mut terminal = Box::new(Terminal::new(rows, cols, config.color_palette));
    let terminal_ptr: *mut Terminal = &mut *terminal;
    let mut parser = vte::Parser::new();
    let mut parser_perform = VteParser::new(terminal_ptr);

    let (mut pty_reader, mut pty_writer) = pty::spawn(&config, cw, ch);
    let proxy = event_loop.create_proxy();

    // Start PTY reader thread
    thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match pty_reader.read(&mut buf) {
                Ok(0) => {
                    let _ = proxy.send_event(AppEvent::PtyExited);
                    break;
                }
                Ok(n) => {
                    let _ = proxy.send_event(AppEvent::PtyOutput(buf[..n].to_vec()));
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => {
                    let _ = proxy.send_event(AppEvent::PtyExited);
                    break;
                }
            }
        }
    });

    event_loop.run(move |event, _target| {
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    std::process::exit(0);
                }
                WindowEvent::Resized(size) => {
                    renderer.resize(size);
                    let (cols, rows) = renderer.terminal_size();
                    unsafe { (*terminal_ptr).resize(rows, cols); }
                    pty_writer.resize(rows as u16, cols as u16);
                }
                WindowEvent::RedrawRequested => {
                    renderer.render(unsafe { &*terminal_ptr });
                    unsafe { (*terminal_ptr).clear_dirty(); }
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    handle_key_input(&mut pty_writer, event);
                }
                WindowEvent::Focused(_) => {}
                _ => {}
            },
            Event::UserEvent(event) => match event {
                AppEvent::PtyOutput(data) => {
                    parser.advance(&mut parser_perform, &data);
                    window.request_redraw();
                }
                AppEvent::PtyExited => {
                    log::info!("pty child exited");
                    std::process::exit(0);
                }
            },
            Event::AboutToWait => {
                window.request_redraw();
            }
            _ => {}
        }
    }).unwrap();
}

fn handle_key_input(writer: &mut pty::PtyWriter, event: KeyEvent) {
    if event.state != ElementState::Pressed {
        return;
    }

    if let Some(text) = event.text {
        if !text.is_empty() {
            let _ = writer.write(text.as_bytes());
            return;
        }
    }

    if let Key::Character(ch) = &event.logical_key {
        if !ch.is_empty() {
            let _ = writer.write(ch.as_bytes());
            return;
        }
    }

    let bytes: Option<Vec<u8>> = match event.logical_key {
        Key::Named(NamedKey::Enter) => Some(vec![b'\r']),
        Key::Named(NamedKey::Backspace) => Some(vec![0x7f]),
        Key::Named(NamedKey::Tab) => Some(vec![b'\t']),
        Key::Named(NamedKey::Escape) => Some(vec![0x1b]),
        Key::Named(NamedKey::ArrowUp) => Some(vec![0x1b, b'[', b'A']),
        Key::Named(NamedKey::ArrowDown) => Some(vec![0x1b, b'[', b'B']),
        Key::Named(NamedKey::ArrowRight) => Some(vec![0x1b, b'[', b'C']),
        Key::Named(NamedKey::ArrowLeft) => Some(vec![0x1b, b'[', b'D']),
        Key::Named(NamedKey::Home) => Some(vec![0x1b, b'[', b'H']),
        Key::Named(NamedKey::End) => Some(vec![0x1b, b'[', b'F']),
        Key::Named(NamedKey::PageUp) => Some(vec![0x1b, b'[', b'5', b'~']),
        Key::Named(NamedKey::PageDown) => Some(vec![0x1b, b'[', b'6', b'~']),
        Key::Named(NamedKey::Insert) => Some(vec![0x1b, b'[', b'2', b'~']),
        Key::Named(NamedKey::Delete) => Some(vec![0x1b, b'[', b'3', b'~']),
        Key::Named(NamedKey::F1) => Some(vec![0x1b, b'O', b'P']),
        Key::Named(NamedKey::F2) => Some(vec![0x1b, b'O', b'Q']),
        Key::Named(NamedKey::F3) => Some(vec![0x1b, b'O', b'R']),
        Key::Named(NamedKey::F4) => Some(vec![0x1b, b'O', b'S']),
        Key::Named(NamedKey::F5) => Some(vec![0x1b, b'[', b'1', b'5', b'~']),
        Key::Named(NamedKey::F6) => Some(vec![0x1b, b'[', b'1', b'7', b'~']),
        Key::Named(NamedKey::F7) => Some(vec![0x1b, b'[', b'1', b'8', b'~']),
        Key::Named(NamedKey::F8) => Some(vec![0x1b, b'[', b'1', b'9', b'~']),
        Key::Named(NamedKey::F9) => Some(vec![0x1b, b'[', b'2', b'0', b'~']),
        Key::Named(NamedKey::F10) => Some(vec![0x1b, b'[', b'2', b'1', b'~']),
        Key::Named(NamedKey::F11) => Some(vec![0x1b, b'[', b'2', b'3', b'~']),
        Key::Named(NamedKey::F12) => Some(vec![0x1b, b'[', b'2', b'4', b'~']),
        _ => None,
    };

    if let Some(data) = bytes {
        let _ = writer.write(&data);
    }
}
