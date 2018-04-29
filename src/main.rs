#[macro_use]
extern crate chan;
extern crate termion;
extern crate tui;
extern crate discord;
extern crate rpassword;

use std::thread;
use std::sync::{Arc, Mutex};

use rpassword::read_password;
use std::vec::Vec;
use std::fs::File;
use std::io::Read;
use std::io;
use std::time;
use std::cmp::{max, min};

use discord::model::Message;
use discord::Discord;

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


static selectedBorder: Style = Style {
    fg: Color::Green,
    bg: Color::Reset,
    modifier: Modifier::Bold
};
static defaultBorder: Style = Style {
    fg: Color::Gray,
    bg: Color::Reset,
    modifier: Modifier::Reset
};


struct MockMessage {
    username: String,
    content: String,
}

#[derive(Debug)]
enum Mode {
    Normal,
    MessageInsert,
    ChannelSelect,
    ServerSelect,
    Command,
    Exiting
}
struct AppState {
    size: Rect,
    messages: Vec<MockMessage>,
    content: String,
    offset: usize,
    servers: Vec<Server>,
    active_server: usize,
    mode: Mode,
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
    fn send_message(&self, discord_chan: &chan::Sender<MsgToDiscord>, content: String) {
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
        // self.to_provider.send(MsgToDiscord::Echo(self.content.clone()));
        let provider = &self.to_provider;
        let text = self.content.clone();
        let active_server = &self.servers[self.active_server];
        let active_channel = &active_server.channels[active_server.active_channel];
        active_channel.send_message(provider, text);
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
    fn quit(&mut self) {

        self.mode = Mode::Exiting;
        self.to_provider.send(MsgToDiscord::Logout);
        
    }
    fn set_servers(&mut self, servers: Vec<discord::model::ServerInfo>) {
        self.servers.clear();

        let mut mut_servers = servers.to_vec();
        mut_servers.reverse();
        for server_info in mut_servers.iter() {
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
                name: format!("{} ({})", d_channel.name, d_channel.kind.name()),
                id: d_channel.id,
                messages: vec!(),
            }
        }).collect();
    }
    fn handle_key(&mut self, key: Key) {
        match self.mode {
            Mode::Normal => {
                match key {
                    Key::Char('i') => {self.mode = Mode::MessageInsert},
                    Key::Char(':') => {self.mode = Mode::Command},
                    Key::Char('s') => {self.mode = Mode::ServerSelect},
                    Key::Char('c') => {self.mode = Mode::ChannelSelect},

                    //Key::Char('k') => self.mode = Mode::Command,
                    //Key::Char('j') => self.mode = Mode::Command,
                    _ => ()
                }
            },
            Mode::MessageInsert => {
                match key {
                    Key::Char('\n') => {
                        self.send_message();
                    }
                    Key::Char(chr) => {
                        self.add_character(chr);
                    }
                    Key::Backspace => {
                        self.remove_character();
                    }
                    Key::Esc => {self.mode = Mode::Normal}
                    _ => ()
                }
            },
            Mode::ChannelSelect => {
                match key {
                    Key::Esc => {self.mode = Mode::Normal}
                    _ => ()
                }},
            Mode::ServerSelect => {
                match key {
                    Key::Esc => {self.mode = Mode::Normal}
                    _ => ()
                }
                },
            Mode::Command => {
                match key {
                    Key::Esc => {self.mode = Mode::Normal}
                    Key::Char('q') => self.quit(),
                    _ =>self.to_provider.send(MsgToDiscord::Echo(format!("Unsupported key for {:?} mode: {:?}", self.mode, key)))
            }},
            _ => ()
        }
    }
    fn store_message(&mut self, message: discord::model::Message) {
        let channel_id = message.channel_id;
        for server in self.servers.iter_mut() {
            for channel in server.channels.iter_mut() {
                if channel.id == channel_id {
                    channel.messages.push(message);
                    return;
                }
            }
        }
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

fn read_token() -> Option<String> {
    let mut data = String::new();
    let mut f = match File::open("./token") {
        Ok(x) => x,
        Err(x) => {
            println!("Couldn't read token file.");
            return None;
        }
    };

    f.read_to_string(&mut data).expect("Unable to read string");
    Some(data)
}

fn main() {
    let discord: Discord = match read_token() {
        None => {
            println!("Check readme to see how to save a token for next time.");
            println!("Falling back to email/pw login.");
            let mut email = String::new();
            println!("Email:");
            io::stdin()
            .read_line(&mut email)
            .expect("failed to read from stdin");
            println!("Password:");
            let pw = read_password().unwrap();
            Discord::new(&email, &pw).expect("Failed to log in.")
        },
        Some(user_token) => match Discord::from_user_token(&user_token) {
                Ok(discord_client) => discord_client,
                Err(error) => {
                    panic!("Login Failed: {}", error);
                }
            }
    };
    let backend = RawBackend::new().unwrap();

    let channel_to_discord = chan::async();
    let channel_from_discord = chan::async();
    // give provider the from_discord sender and the to_discord receiver
    let provider = DiscordProvider::init(discord, (
        channel_from_discord.0.clone(),
        channel_to_discord.1.clone(),
    ));
    thread::spawn(|| { provider.start_provider(); });

 
    let mut terminal = Terminal::new(backend).unwrap();
 
    let mut app_state = AppState {
        size: Rect::default(),
        messages: vec![],
        content: String::from(""),
        offset: 0,
        active_server: 0,
        servers: vec![],
        mode: Mode::Normal,
        to_provider: channel_to_discord.0.clone(),
        from_provider: channel_from_discord.1.clone(),
    };
    app_state.get_servers();
    let dummy_channel = Channel {
        name: String::from("Loading..."),
        id: discord::model::ChannelId {
            0: 1,
        },
        messages: vec![],
    };

    let dummy_server = Server {
        channels: vec![dummy_channel],
        active_channel: 0,
        server_info: discord::model::ServerInfo {
            id: discord::model::ServerId {
                0: 1234,
            },
            name: String::from("Loading..."),
            icon: None,
            owner: true,
            permissions: discord::model::permissions::Permissions::empty(),
        },
    };

    app_state.servers.push(dummy_server);
    let terminal = Arc::new(Mutex::new(terminal));
    let app_state = Arc::new(Mutex::new(app_state));

    let term = Arc::clone(&terminal);
    let state = Arc::clone(&app_state);

    term.lock().unwrap().clear().unwrap();
    draw(&mut term.lock().unwrap(), &mut state.lock().unwrap());

    let stdin = io::stdin();

    let term = Arc::clone(&terminal);
    let state = Arc::clone(&app_state);

    thread::spawn(move || {

        for c in stdin.keys() {
            let mut terminal = term.lock().unwrap();
            let mut app_state = state.lock().unwrap();

            let evt = c.unwrap();
            app_state.handle_key(evt);

            match app_state.mode {
                Mode::Command => terminal.show_cursor(),
                Mode::MessageInsert => terminal.show_cursor(),
                Mode::Exiting => {
                    terminal.show_cursor();
                    terminal.clear();
                    std::process::exit(0);
                }
                _ => terminal.hide_cursor()
            };

            /*
            match evt {
                event::Key::Char('\t') => {
                    app_state.selected_tab = match app_state.selected_tab {
                        TabSelect::Servers => TabSelect::Channels,
                        TabSelect::Channels => TabSelect::Servers,
                    }
                }
                event::Key::Down => {
                    match app_state.selected_tab {
                        TabSelect::Servers => app_state.next_server(),
                        TabSelect::Channels => app_state.active_server().next_channel(),
                    }
                    terminal.draw();
                },
                event::Key::Up => {
                    match app_state.selected_tab {
                        TabSelect::Servers => app_state.prev_server(),
                        TabSelect::Channels => app_state.active_server().prev_channel(),
                    }
                    terminal.draw();
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
            */
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
                            app_state.store_message(message);
                        },
						MsgFromDiscord::Exit => {println!("Got exit msg"); break;},
                        _ => {
                            app_state.messages.push(MockMessage{
                                username: String::from("DiscordProvider"),
                                content: String::from(format!("{:?}", message)),
                            })
                        }
                    }
                }


				let size = terminal.size().unwrap();
				if size != app_state.size {
					terminal.resize(size).unwrap();
					app_state.size = size;
				}
                draw(&mut terminal, &mut app_state);
            },
        };
    }

    let term = Arc::clone(&terminal);
    let mut t = term.lock().unwrap();
    t.show_cursor().unwrap();
    t.clear().unwrap();
    std::process::exit(0);
}

fn draw(t: &mut Terminal<RawBackend>, state: &AppState) {
    let size = t.size().unwrap();
    let channel_name = "temp1";

    Group::default().direction(Direction::Vertical)
        .sizes(&[Size::Min(1), Size::Fixed(1)])
        .render(t, &size, |t, chunks| {
            Group::default()
                .direction(Direction::Horizontal)
                .sizes(&[Size::Percent(20), Size::Percent(80)])
                .render(t, &chunks[0], |t, chunks| {
                    draw_left(t, state, &chunks[0]);
                    draw_messagePane(t, state, &chunks[1]);
                });
            Paragraph::default().text(&format!("Mode: {:?}", state.mode)).render(t, &chunks[1]);
        });

    t.draw();
}

fn draw_messagePane(t: &mut Terminal<RawBackend>, state: &AppState, area: &Rect) {
    let style = Style::default().fg(Color::Yellow);
    let mut channel_name = "temp2";

    Group::default()
        .direction(Direction::Vertical)
        .sizes(&[Size::Percent(90), Size::Min(0)])
        .render(t, area, |t, chunks| {
            let active_server = &state.servers[state.active_server];
            let mut msgs: Vec<discord::model::Message> = vec!();

            if (active_server.channels.len() > 0) {
                let active_channel = &active_server.channels[active_server.active_channel];
                msgs = active_channel.messages.clone();
                channel_name = &active_channel.name[..];
            }

            let n = msgs.len();
            let nm = chunks[1].height as usize;
            let left_bound: usize = match n.checked_sub(nm) {
                Some(x) => x,
                None => 0,
            };

            let msgs = msgs[0..n].iter().map( |msg| {
                Item::StyledData(
                    format!("{}: {}", &msg.author.name[..], &msg.content[..]),
                    &style,
                )
            });


            List::new(msgs)
                .block(Block::default().borders(Borders::ALL).title(&format!("#{}", channel_name)[..]))
                .render(t, &chunks[0]);
            match state.mode {
                Mode::MessageInsert => {
                     Paragraph::default()
                        .text(&state.content[..])
                        .block(Block::default().borders(Borders::ALL).title(&format!("Message #{}", channel_name)))
                        .render(t, &chunks[1]);
                }
                _ => {
                    Paragraph::default().text("help goes here")
                        .block(Block::default())
                        .render(t, &chunks[1]);
                }
            }
        });
}

fn draw_left(t: &mut Terminal<RawBackend>, state: &AppState, area: &Rect) {
    Group::default()
        .direction(Direction::Vertical)
        .sizes(&[Size::Percent(50), Size::Percent(50)])
        .render(t, area, |t, chunks| {


            SelectableList::default()
                .block(Block::default().borders(Borders::ALL).title("Servers").border_style(selectedBorder))
                .items(&state.servers)
                .select(state.active_server)
                .highlight_style(Style::default().fg(Color::Green).modifier(Modifier::Bold))
                /*
                 *.highlight_symbol(
                 *    match state.selected_tab {
                 *    TabSelect::Servers=>">",
                 *    _ => " "
                 *})
                 */
                .render(t, &chunks[0]);

            SelectableList::default()
                .block(Block::default().borders(Borders::ALL).title("Channels"))
                .items(&state.servers[state.active_server].channels)
                .select(state.servers[state.active_server].active_channel)
                .highlight_style(Style::default().fg(Color::Green).modifier(Modifier::Bold))
                /*
                 *.highlight_symbol(
                 *    match state.selected_tab {
                 *    TabSelect::Channels=>">",
                 *    _ => " "
                 *})
                 */
                .render(t, &chunks[1]);

        });
}
