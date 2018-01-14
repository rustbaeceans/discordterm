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
use std::time;

use discord::model::Message;

use termion::event;
use termion::event::Key;
use termion::input::TermRead;

use tui::Terminal;
use tui::backend::RawBackend;
use tui::widgets::{Widget, Block, Borders, Item, List, SelectableList, Paragraph};
use tui::layout::{Group, Size, Rect, Direction};
use tui::style::{Color, Modifier, Style};

mod discord_provider;
use discord_provider::{DiscordProvider, Msg, MsgToDiscord};

struct MockMessage {
    username: String,
    content: String,
}

enum TabSelect {
    Channels,
    Servers,
}

struct AppState {
    messages: Vec<MockMessage>,
    content: String,
    channels: Vec<String>,
    selected_channel: usize,
    servers: Vec<String>,
    selected_server: usize,
    selected_tab: TabSelect,
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
    for i in 1..5 {
        provider_chan.0.send(Msg::ToDiscord(MsgToDiscord::Echo(String::from("Test!"))));
    }
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
        channels: vec![String::from("general"), String::from("baes-only")], // TODO: Add real channels
        selected_channel: 0,
        servers: vec![String::from("Server 1"), String::from("Server 2")], // TODO: Add real servers
        selected_server: 0,
        selected_tab: TabSelect::Channels,
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
                event::Key::Char('\t') => {
                    app_state.selected_tab = match app_state.selected_tab {
                        TabSelect::Servers => TabSelect::Channels,
                        TabSelect::Channels => TabSelect::Servers,
                    }
                },
                event::Key::Char(chr) => {
                    app_state.add_character(chr);
                    terminal.show_cursor().unwrap();
                },
                event::Key::Backspace => {
                    app_state.remove_character();
                },
                event::Key::Down => {
                    match app_state.selected_tab {
                        TabSelect::Servers => {
                            app_state.selected_server += 1;
                            if app_state.selected_server > app_state.servers.len() - 1 {
                                app_state.selected_server = 0;
                            }
                        }
                        TabSelect::Channels => {
                            app_state.selected_channel += 1;
                            if app_state.selected_channel > app_state.channels.len() - 1 {
                                app_state.selected_channel = 0;
                            }
                        }
                        _ => {}
                    };
                    
                },
                event::Key::Up => {
                    match app_state.selected_tab {
                        TabSelect::Servers => {
                            if app_state.selected_server > 0 {
                                app_state.selected_server -= 1;
                            } else {
                                app_state.selected_server = app_state.servers.len() - 1;
                            }
                        }
                        TabSelect::Channels => {
                            if app_state.selected_channel > 0 {
                                app_state.selected_channel -= 1;
                            } else {
                                app_state.selected_channel = app_state.channels.len() - 1;
                            }
                        }
                        _ => {}
                    };
                    
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

    let term = Arc::clone(&terminal);
    let state = Arc::clone(&app_state);

    let term = Arc::clone(&terminal);
    let state = Arc::clone(&app_state);
    thread::spawn(move || {
        loop {
            thread::sleep(time::Duration::from_secs(1));
            let mut terminal = term.lock().unwrap();
            let mut app_state = state.lock().unwrap();

            app_state.messages.push(MockMessage{
                 username:String::from("test"), content: String::from("hey")
            });
        }
    });

    let term = Arc::clone(&terminal);
    let state = Arc::clone(&app_state);
    let dp_rx = provider_chan.1.clone();
    loop {
        chan_select! {
            default => {
                thread::sleep_ms(10);
            },
            rx.recv() => {
                break;
            },
            dp_rx.recv() -> val => {
                let mut terminal = term.lock().unwrap();
                let mut app_state = state.lock().unwrap();
                app_state.messages.push(MockMessage{
                     username:String::from("DiscordProvider"), content: String::from(format!("-> {:?}", val))
                });
                draw(&mut terminal, &mut app_state);
            },
        };
    }
    let term = Arc::clone(&terminal);
    let mut t = term.lock().unwrap();
    t.show_cursor().unwrap();
    t.clear().unwrap();
}

fn draw(t: &mut Terminal<RawBackend>, state: &AppState) {
    let size = t.size().unwrap();
    let channel_name = &state.channels[state.selected_channel];

    Group::default()
        .direction(Direction::Vertical)
        .sizes(&[Size::Min(0), Size::Fixed(3)])
        .render(t, &size, |t, chunks| {

            draw_top(t, state, &chunks[0]);

            Paragraph::default()
                .text(&state.content[..])
                .block(Block::default().borders(Borders::ALL).title("Message #channel")) // &format!("Message #{}", channel_name) <-- TODO: Figure out why this makes it slower
                .render(t, &chunks[1]);
        });

    t.draw();
}

fn draw_top(t: &mut Terminal<RawBackend>, state: &AppState, area: &Rect) {
    let style = Style::default().fg(Color::Yellow);
    let channel_name = &state.channels[state.selected_channel];

    Group::default()
        .direction(Direction::Horizontal)
        .sizes(&[Size::Percent(20), Size::Min(0)])
        .render(t, area, |t, chunks| {
            let msgs = state.messages.iter().map( |msg| {
                Item::StyledData(
                    format!("{}: {}", &msg.username[..], &msg.content[..]),
                    &style,
                )
            });

            draw_left(t, state, &chunks[0]);

            List::new(msgs)
                .block(Block::default().borders(Borders::ALL).title("#channel")) // &format!("#{}", channel_name) <-- TODO: Figure out why this makes it slower
                .render(t, &chunks[1]);
        });
}

fn draw_left(t: &mut Terminal<RawBackend>, state: &AppState, area: &Rect) {
    Group::default()
        .direction(Direction::Vertical)
        .sizes(&[Size::Percent(50), Size::Percent(50)])
        .render(t, area, |t, chunks| {
            SelectableList::default()
                .block(Block::default().borders(Borders::ALL).title("Servers"))
                .items(&state.servers)
                .select(state.selected_server)
                .highlight_style(Style::default().fg(Color::Yellow).modifier(Modifier::Bold))
                .highlight_symbol(">")
                .render(t, &chunks[0]);

            SelectableList::default()
                .block(Block::default().borders(Borders::ALL).title("Channels"))
                .items(&state.channels)
                .select(state.selected_channel)
                .highlight_style(Style::default().fg(Color::Yellow).modifier(Modifier::Bold))
                .highlight_symbol(">")
                .render(t, &chunks[1]);
        });
}
