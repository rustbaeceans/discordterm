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
use std::cmp::{max, min};

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
use discord_provider::{DiscordProvider, MsgToDiscord, MsgFromDiscord};

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
    offset: usize,
    servers: Vec<Server>,
    active_server: usize,
    selected_tab: TabSelect,
    to_provider: chan::Sender<MsgToDiscord>,
    from_provider: chan::Receiver<MsgFromDiscord>,
}

#[derive(Clone)]
struct Server {
    channels: Vec<Channel>,
    active_channel: usize,
    server_info: discord::model::ServerInfo,
}

impl AsRef<str> for Server {
    fn as_ref(&self) -> &str {
       &self.server_info.name
    }
}
#[derive(Clone)]
struct Channel {
    name: String,
    id: discord::model::ChannelId,
    messages: Vec<discord::model::Message>,
}

impl Channel {
    fn send_message(&self, discord_chan: chan::Sender<MsgToDiscord>, content: String) {
        let message = MsgToDiscord::SendMessage(
            self.id,
            content,
        );
        discord_chan.send(message);
    }
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
        let end = self.content.len();
        self.content = format!("{}{}{}", &self.content[0..self.offset], content_to_append, &self.content[self.offset..end]);
        self.offset = min(self.content.len(), self.offset + 1);
    }
    fn remove_character(&mut self) {
        let n = self.content.len();

        let left_bound = match self.offset.checked_sub(1) {
            Some(x) => x,
            None => 0,
        };

        let right_bound = min(n, self.offset + 1);

        if (n != 0) {
            self.content = format!("{}{}", &self.content[..left_bound], &self.content[self.offset..n]);
            self.offset = left_bound;
        }
    }
    fn send_message(&mut self) {
        self.to_provider.send(MsgToDiscord::Echo(self.content.clone()));
        self.content = String::from("");
        self.offset = 0;
    }
    fn next_server(&mut self) {
        let new_index = (self.active_server + 1) % self.servers.len();
        self.active_server = new_index;
    }
    fn prev_server(&mut self) {
        if self.active_server > 0 {
            self.active_server -= 1;
        } else {
            self.active_server = self.servers.len() - 1;
        }
    }
    fn active_server(&mut self) -> &mut Server {
        &mut self.servers[self.active_server]
    }
    fn get_servers(&self) {
        self.to_provider.send(MsgToDiscord::GetServers);
    }
    fn set_servers(&mut self, servers: Vec<discord::model::ServerInfo>) {
        self.servers.clear();
        for server_info in servers.iter() {
            self.servers.push(Server{
                channels: Vec::new(),
                active_channel: 0,
                server_info: server_info.clone(),
            });
            self.to_provider.send(MsgToDiscord::GetChannels(server_info.id));
        };
    }
    fn set_channels(&mut self, owner: discord::model::ServerId, channels: Vec<discord::model::PublicChannel>) {
        let temp = self.servers.clone();
        let (i, owning_server) = temp.iter().enumerate().find(|&(i, server)| {
            server.server_info.id == owner
        }).unwrap();

        self.servers[i].channels = channels.iter().map(|d_channel| {
            let d_channel = d_channel.clone();
            Channel {
                name: d_channel.name,
                id: d_channel.id,
                messages: vec!(),
            }
        }).collect();
    }
}

impl Server {
    fn next_channel(&mut self) {
        if self.channels.len() == 0 {
            self.active_channel = 0;
        } else {
           let new_index = (self.active_channel + 1) % self.channels.len();
            self.active_channel = new_index; 
        }
    }
    fn prev_channel(&mut self) {
        if self.active_channel > 0 {
            self.active_channel -= 1;
        } else {
            if (self.channels.len() == 0) {
                self.active_channel = 0;
            } else {
                self.active_channel = self.channels.len() - 1;
            }
        }
    }
    fn active_channel(&mut self) -> &mut Channel {
        &mut self.channels[self.active_channel]
    }
}

fn read_token() -> String {
    let mut data = String::new();
    let mut f = match File::open("./token") {
        Ok(x) => x,
        Err(x) => {
            println!("Couldn't log in.");
            return String::from("0");
        }
    };

    f.read_to_string(&mut data).expect("Unable to read string");
    data
}

fn main() {

    let backend = RawBackend::new().unwrap();

    let channel_to_discord = chan::async();
    let channel_from_discord = chan::async();
    // give provider the from_discord sender and the to_discord receiver
    let provider = DiscordProvider::init(read_token(), (
        channel_from_discord.0.clone(),
        channel_to_discord.1.clone(),
    ));
    thread::spawn(|| { provider.start_provider(); });

    let example_message3 = String::from("test");
    channel_to_discord.0.send(MsgToDiscord::SendMessage(
        discord::model::ChannelId {
            0: 402096812296503298,
        },
        example_message3,
    ));
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app_state = AppState {
        messages: vec![],
        content: String::from(""),
        offset: 0,
        active_server: 0,
        servers: vec![],
        selected_tab: TabSelect::Channels,
        to_provider: channel_to_discord.0.clone(),
        from_provider: channel_from_discord.1.clone(),
    };
    app_state.get_servers();
    let test_channel1 = Channel {
        name: String::from("Test Channel S1 - 1"),
        id: discord::model::ChannelId {
            0: 1,
        },
        messages: vec![],
    };

    let test_channel2 = Channel {
        name: String::from("Test Channel S1 - 2"),
        id: discord::model::ChannelId {
            0: 2,
        },
        messages: vec![],
    };

    let test_channel3 = Channel {
        name: String::from("Test Channel S2 - 3"),
        id: discord::model::ChannelId {
            0: 3,
        },
        messages: vec![],
    };

    let test_channel4 = Channel {
        name: String::from("Test Channel S2 - 4"),
        id: discord::model::ChannelId {
            0: 4,
        },
        messages: vec![],
    };


    let test_server1 = Server {
        channels: vec![test_channel1, test_channel2],
        active_channel: 0,
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
        channels: vec![test_channel3, test_channel4],
        active_channel: 0,
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
                }
                event::Key::Char('\t') => {
                    app_state.selected_tab = match app_state.selected_tab {
                        TabSelect::Servers => TabSelect::Channels,
                        TabSelect::Channels => TabSelect::Servers,
                    }
                }
                event::Key::Char(chr) => {
                    app_state.add_character(chr);
                    terminal.show_cursor().unwrap();
                }
                event::Key::Backspace => {
                    app_state.remove_character();
                }
                event::Key::Down => {
                    match app_state.selected_tab {
                        TabSelect::Servers => app_state.next_server(),
                        TabSelect::Channels => app_state.active_server().next_channel(),
                    }
                },
                event::Key::Up => {
                    match app_state.selected_tab {
                        TabSelect::Servers => app_state.prev_server(),
                        TabSelect::Channels => app_state.active_server().prev_channel(),
                    }
                },
                event::Key::Left => {
                    app_state.offset = match app_state.offset.checked_sub(1) {
                        Some(x) => x,
                        None => 0,
                    };
                },
                event::Key::Right => {
                    app_state.offset = min(app_state.content.len(), app_state.offset + 1);
                }
                event::Key::Ctrl('c') => {
                    tx.send(true);
                    break;
                }
                _ => {}
            }
            draw(&mut terminal, &mut app_state);
        }
    });

    let term = Arc::clone(&terminal);
    let state = Arc::clone(&app_state);
    let rx_from_pvdr = channel_from_discord.1.clone();
    loop {
        chan_select! {
            default => {
                thread::sleep_ms(10);
            },
            rx.recv() => {
                break;
            },
            rx_from_pvdr.recv() -> val => {
                let mut terminal = term.lock().unwrap();
                let mut app_state = state.lock().unwrap();

                if let Some(message) = val {
                    match message {
                        MsgFromDiscord::Servers(servers) => {
                            app_state.set_servers(servers);
                        },
                        MsgFromDiscord::Channels(server_id, channels) => {
                            app_state.set_channels(server_id, channels)
                        },
                        MsgFromDiscord::ChatMsg(message) => {
                            app_state.messages.push(MockMessage{
                                username: message.author.name,
                                content: message.content,
                            })
                        },
                        MsgFromDiscord::EchoResponse(message) => {
                            app_state.messages.push(MockMessage{
                                username: String::from("me"),
                                content: message,
                            })
                        },
                    }
                }

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

            let n = state.messages.len();
            let nm = chunks[1].height as usize;
            let left_bound: usize = match n.checked_sub(nm) {
                Some(x) => x,
                None => 0,
            };

            let msgs = state.messages[left_bound..n].iter().map( |msg| {
                Item::StyledData(
                    format!("{}: {}", &msg.username[..], &msg.content[..]),
                    &style,
                )
            });

            draw_left(t, state, &chunks[0]);


            List::new(msgs)
                .block(Block::default().borders(Borders::ALL).title(&format!("#{}", channel_name)[..]))
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
                .select(state.servers[state.active_server].active_channel)
                .highlight_style(Style::default().fg(Color::Yellow).modifier(Modifier::Bold))
                .highlight_symbol(">")
                .render(t, &chunks[1]);

        });
}
