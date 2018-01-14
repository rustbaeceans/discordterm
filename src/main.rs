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
    servers: Vec<Server>,
    active_server: usize,
    selected_tab: TabSelect,
}

struct Server {
    channels: Vec<Channel>,
    server_info: discord::model::ServerInfo,
}

impl AsRef<str> for Server {
    fn as_ref(&self) -> &str {
       &self.server_info.name
    }
}

struct Channel {
    name: String,
    id: discord::model::ChannelId,
    messages: Vec<discord::model::Message>,
}

impl AsRef<str> for Channel {
    fn as_ref(&self) -> &str {
       &self.name
    }
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
        active_server: 0,
        servers: vec!(),
        selected_tab: TabSelect::Channels,
    };

    let test_channel1 = Channel {
        name: String::from("Test Channel S1 - 1"),
        id: discord::model::ChannelId {
            0: 1,
        },
        messages: vec!(),
    };

    let test_channel2 = Channel {
        name: String::from("Test Channel S1 - 2"),
        id: discord::model::ChannelId {
            0: 2,
        },
        messages: vec!(),
    };

    let test_channel3 = Channel {
        name: String::from("Test Channel S2 - 3"),
        id: discord::model::ChannelId {
            0: 3,
        },
        messages: vec!(),
    };

    let test_channel4 = Channel {
        name: String::from("Test Channel S2 - 4"),
        id: discord::model::ChannelId {
            0: 4,
        },
        messages: vec!(),
    };


    let test_server1 = Server {
        channels: vec!(test_channel1, test_channel2),
        server_info: discord::model::ServerInfo {
            id: discord::model::ServerId {
                0: 1234,
            },
            name: String::from("Test Server 1"),
            icon: None,
            owner: true,
            permissions: discord::model::permissions::Permissions::empty(),
        },
    };

    let test_server2 = Server {
        channels: vec!(test_channel3, test_channel4),
        server_info: discord::model::ServerInfo {
            id: discord::model::ServerId {
                0: 12345,
            },
            name: String::from("Test Server 2"),
            icon: None,
            owner: true,
            permissions: discord::model::permissions::Permissions::empty(),
        },
    };


    app_state.servers.push(test_server1);
    app_state.servers.push(test_server2);
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
                    let current_index = app_state.active_server;
                    let new_index = (current_index + 1) % app_state.servers.len();
                    app_state.active_server = new_index;
                },
                event::Key::Up => {
                    if app_state.active_server > 0 {
                        app_state.active_server -= 1;
                    } else {
                        app_state.active_server = app_state.servers.len();
                    }
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
    let channel_name = "temp1";

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
    let channel_name = "temp2";

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
                .select(state.active_server)
                .highlight_style(Style::default().fg(Color::Yellow).modifier(Modifier::Bold))
                .highlight_symbol(">")
                .render(t, &chunks[0]);

            SelectableList::default()
                .block(Block::default().borders(Borders::ALL).title("Channels"))
                .items(&state.servers[state.active_server].channels)
                .select(0)
                .highlight_style(Style::default().fg(Color::Yellow).modifier(Modifier::Bold))
                .highlight_symbol(">")
                .render(t, &chunks[1]);

        });
}
