#[macro_use]
extern crate chan;
extern crate termion;
extern crate tui;

use std::thread;
use std::sync::{Arc, Mutex};

use std::io;

use termion::event;
use termion::input::TermRead;

use tui::Terminal;
use tui::backend::RawBackend;
use tui::widgets::{Widget, Block, Borders, Paragraph};
use tui::layout::{Group, Size, Direction};

struct AppState {
    content: String,
}

impl AppState {
    fn add_character(&mut self, chr: char) {
        let mut content_to_append = String::new();
        content_to_append.push(chr);
        self.content = format!("{}{}", self.content, content_to_append);
    }
}

fn main() {

    let backend = RawBackend::new().unwrap();
    let mut terminal = Terminal::new(backend).unwrap();

    let mut app_state = AppState {
        content: String::from(""),
    };

    let terminal = Arc::new(Mutex::new(terminal));
    let app_state = Arc::new(Mutex::new(app_state));


    let term = Arc::clone(&terminal);
    let state = Arc::clone(&app_state);

    term.lock().unwrap().clear().unwrap();
    draw(&mut term.lock().unwrap(), &mut state.lock().unwrap());

    let stdin = io::stdin();
    let (tx, rx) = chan::async();

    let term = Arc::clone(&terminal);
    let state = Arc::clone(&state);
    thread::spawn(move || {
        let tx = tx.clone();

        for c in stdin.keys() {
            let mut terminal = term.lock().unwrap();
            let mut app_state = state.lock().unwrap();

            let evt = c.unwrap();
            match evt {
                event::Key::Char(chr) => {
                    app_state.add_character(chr);
                },
                event::Key::Ctrl('c') => {
                    tx.send(true);
                    break;
                },
                _ => {},
            }
            
            draw(&mut terminal, &mut app_state);
        }
    });

    loop {
        chan_select! {
            default => {},
            rx.recv() => {
                break;
            }
        }
    }
    let term = Arc::clone(&terminal);
    let mut t = term.lock().unwrap();
    t.show_cursor().unwrap();
    t.clear().unwrap();

}

fn draw(t: &mut Terminal<RawBackend>, state: &mut AppState) {
    let size = t.size().unwrap();
    let state = &*state;

    Paragraph::default()
        .text(&state.content[..])
        .block(Block::default().borders(Borders::ALL).title("Terminal"))
        .render(t, &size);

    t.draw();
}
