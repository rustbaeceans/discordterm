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
struct MockMessage<'a> {
    username: &'a str,
    content: &'a str,
} 

struct AppState<'a> {
    messages: Vec<MockMessage<'a>>,
}

fn read_token() -> String {
  let mut data = String::new();
    let mut f = File::open("./token").expect("Unable to open file");
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
        username: "Namtsua",
        content: "I love fidget spinners",
    };

    let example_message2 = MockMessage {
        username: "harbo",
        content: "Let's relax",
    };

    let mut terminal = Terminal::new(backend).unwrap();
    let mut app_state = AppState {
        messages: vec!(example_message, example_message2),
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
                Key::Ctrl('c') => { tx.send(true); break; },
                _ => {},
            }
            draw(&mut terminal, &mut app_state);
        }
    });

    let dp_rx = provider_chan.1;
    loop {
        chan_select! {
            default => {},
            rx.recv() => {
                break;
            },
            dp_rx.recv() -> val => {
                 state.lock().unwrap().messages.push(MockMessage{
                     username:"DiscordProvider", content: String::from(format!("{:?}", val))
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

    let msgs = state.messages.iter().map( |msg| {
        Item::StyledData(
            format!("{}: {}", msg.username, msg.content),
            &style,
        )
    });

    List::new(msgs)
        .block(Block::default().borders(Borders::ALL).title("Discord"))
        .render(t, &size);

    t.draw();
}
