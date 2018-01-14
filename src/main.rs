#[macro_use]
extern crate chan;
extern crate termion;
extern crate tui;
extern crate discord;

use std::thread;
use std::sync::{Arc, Mutex};

use std::io;
use std::vec::Vec;
use std::fs::File;
use std::io::Read;

use discord::model::Message;

use termion::event;
use termion::event::Key;
use termion::input::TermRead;

use tui::Terminal;
use tui::backend::RawBackend;
use tui::widgets::{Widget, Block, Borders, Item, List, Paragraph};
use tui::layout::{Group, Size, Direction};
use tui::style::{Color, Style};

mod discord_provider;
use discord_provider::{DiscordProvider, Msg, MsgToDiscord};

struct MockMessage {
    username: String,
    content: String,
}

struct AppState {
    messages: Vec<MockMessage>,
    content: String,
}

impl AppState {
    fn add_character(&mut self, chr: char) {
        let mut content_to_append = String::new();
        content_to_append.push(chr);
        self.content = format!("{}{}", self.content, content_to_append);
    }
    fn remove_character(&mut self) {
        let n = self.content.chars().count();
        if (n != 0) {
            self.content = String::from(&self.content[..n-1]);
        }
    }
    fn send_message(&mut self) {
        self.content = String::from("");
    }
}

fn read_token() -> String {
  let mut data = String::new();
    let mut f = match File::open("./token") {
        Ok(x) => x,
        Err(x) => {println!("Couldn't log in."); return String::from("0");}
    };
    
    f.read_to_string(&mut data).expect("Unable to read string");
    data
}

fn main() {

    let backend = RawBackend::new().unwrap();

    let provider_chan = chan::async();
    let provider = DiscordProvider::init(read_token(), provider_chan.clone());
    thread::spawn(move || {
        provider.outgoing_loop();
    });
    provider_chan.0.send(Msg::ToDiscord(MsgToDiscord::Echo(String::from("Test!"))));
    let example_message = MockMessage {
        username: String::from("Namtsua"),
        content: String::from("I love fidget spinners"),
    };

    let example_message2 = MockMessage {
        username: String::from("harbo"),
        content: String::from("Let's relax"),
    };

    let mut terminal = Terminal::new(backend).unwrap();
    let mut app_state = AppState {
        messages: vec!(example_message, example_message2),
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
    let state = Arc::clone(&app_state);

    thread::spawn(move || {
        let tx = tx.clone();

        for c in stdin.keys() {
            let mut terminal = term.lock().unwrap();
            let mut app_state = state.lock().unwrap();

            let evt = c.unwrap();
            match evt {
                event::Key::Char('\n') => {
                    app_state.send_message();
                },
                event::Key::Char(chr) => {
                    app_state.add_character(chr);
                    terminal.show_cursor().unwrap();

                },
                event::Key::Backspace => {
                    app_state.remove_character();
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

    let dp_rx = provider_chan.1;
    let state = Arc::clone(&app_state);
    loop {
// app_state.messages.push(MockMessage{
//                      username:String::from("test"), content: String::from("hey")
//                 });
        chan_select! {
            default => {},
            rx.recv() => {
                break;
            },
            dp_rx.recv() -> val => {
                let mut app_state = state.lock().unwrap();
                println!("Adding message to list");
                break;
                app_state.messages.push(MockMessage{
                     username:String::from("DiscordProvider"), content: String::from(format!("-> {:?}", val))
                });
            },
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
    let style = Style::default().fg(Color::Yellow);

    let state = &*state;


    Group::default()
        .direction(Direction::Vertical)
        .sizes(&[Size::Min(0), Size::Fixed(3)])
        .render(t, &size, |t, chunks| {
            let msgs = state.messages.iter().map( |msg| {
                Item::StyledData(
                    format!("{}: {}", &msg.username[..], &msg.content[..]),
                    &style,
                )
            });

            List::new(msgs)
                .block(Block::default().borders(Borders::ALL).title("Discord"))
                .render(t, &chunks[0]);

            Paragraph::default()
                .text(&state.content[..])
                .block(Block::default().borders(Borders::ALL).title("Terminal"))
                .render(t, &chunks[1]);
        });

    t.draw();
}
