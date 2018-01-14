#[macro_use]
extern crate chan;
extern crate termion;
extern crate tui;
extern crate discord;

use std::thread;
use std::sync::{Arc, Mutex};

use std::io;

use termion::event;
use termion::input::TermRead;

use tui::Terminal;
use tui::backend::RawBackend;
use tui::widgets::{Widget, Block, Borders, Paragraph};
use tui::layout::{Group, Size, Direction};

mod discord_provider;

struct AppState {

}

fn main() {

    let backend = RawBackend::new().unwrap();
    let mut terminal = Terminal::new(backend).unwrap();

    let terminal = Arc::new(Mutex::new(terminal));

    let term = Arc::clone(&terminal);
    term.lock().unwrap().clear().unwrap();
    draw(&mut term.lock().unwrap());

    let stdin = io::stdin();
    let (tx, rx) = chan::async();

    let term = Arc::clone(&terminal);
    thread::spawn(move || {
        let tx = tx.clone();

        for c in stdin.keys() {
            let mut terminal = term.lock().unwrap();

            let evt = c.unwrap();
            if evt == event::Key::Char('q') {
                tx.send(true);
                break;
            }
            draw(&mut terminal);
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

fn draw(t: &mut Terminal<RawBackend>) {
    let size = t.size().unwrap();

    Paragraph::default()
        .text("Block")
        .block(Block::default().borders(Borders::ALL).title("Terminal"))
        .render(t, &size);

    t.draw();
}
