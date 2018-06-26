#[macro_use]
extern crate chan;
extern crate termion;
extern crate tui;
extern crate discord;
extern crate rpassword;
extern crate itertools;

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

mod chatwidget;
use chatwidget::ChatWidget;

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

#[derive(Debug, PartialEq, Clone)]
enum Mode {
    Normal,
    TextInput,
    ChannelSelect,
    ServerSelect,
    Command,
    Fzf,
    Exiting
}
struct AppState {
    size: Rect,
    messages: Vec<MockMessage>,
    content: String,
    offset: usize,
    scroll_pos: usize,
    servers: Vec<Server>,
    active_server: usize,
    mode: Mode,
    mode_stack: Vec<Mode>,
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
    fn new( to_provider: chan::Sender<MsgToDiscord>, from_provider: chan::Receiver<MsgFromDiscord>) -> Self {
        AppState {
            size: Rect::default(),
            messages: vec![],
            content: String::from(""),
            offset: 0,
            scroll_pos: 0,
            active_server: 0,
            servers: vec![],
            mode: Mode::Normal,
            mode_stack: vec![],
            to_provider,
            from_provider
        }
    }
    fn add_character(&mut self, chr: char) {
        let mut content_to_append = String::new();
        content_to_append.push(chr);
        let end = self.content.len();
        self.content = format!("{}{}{}", &self.content[0..self.offset], content_to_append, &self.content[self.offset..end]);
        self.offset = min(self.content.len(), self.offset + 1);
    }
    fn remove_character(&mut self) {
        let n = self.content.len();

        let left_bound = self.offset.checked_sub(1).unwrap_or(0);

        let right_bound = min(n, self.offset + 1);

        if (n != 0) {
            self.content = format!("{}{}", &self.content[..left_bound], &self.content[self.offset..n]);
            self.offset = left_bound;
        }
    }
    fn send_message(&mut self, text: String) {
        // self.to_provider.send(MsgToDiscord::Echo(self.content.clone()));
        let provider = &self.to_provider;
        let active_server = &self.servers[self.active_server];
        let active_channel = &active_server.channels[active_server.active_channel];
        active_channel.send_message(provider, text);
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


    fn switch_mode(&mut self, new_mode: Mode){
        let old_mode = self.mode.clone();
        self.mode = new_mode;
        self.mode_stack.push(old_mode);
        match self.mode {
            Mode::Command => self.switch_mode(Mode::TextInput),
            _ => ()
        }
        // todo: state transitions?
    }
    fn prev_mode(&mut self){
        self.mode = self.mode_stack.pop().unwrap_or(Mode::Normal);
         match self.mode {
            //Mode::Command => self.prev_mode(),
            _ => ()
        }
    }
    fn perform_command(&mut self, command: String){
        assert_eq!(self.mode, Mode::Command);
        match command.as_ref() {
            "q" => self.quit(),
            _ => self.print(format!("Unknown command {}", command))
        };
        self.prev_mode()
    }

    fn process_text_input(&mut self) {
        let text = self.content.clone();
        self.content = String::from("");
        self.offset = 0;
        match self.mode {
            Mode::Normal => self.send_message(text),
            Mode::Command => self.perform_command(text),
            _ => panic!("How did we get to {:?} from TextInput? Stack: {:?}", self.mode, self.mode_stack)

        }
    }
    fn handle_key(&mut self, key: Key) {
        match self.mode {
            Mode::Normal => {
                match key {
                    Key::Char('i') => self.switch_mode(Mode::TextInput),
                    Key::Char(':') => self.switch_mode(Mode::Command),
                    Key::Char('s') => self.switch_mode(Mode::ServerSelect),
                    Key::Char('c') => self.switch_mode(Mode::ChannelSelect),
                    //Key::Ctrl('k') => self.switch_mode(Mode::Fzf),
                    //Key::Char('/') => self.switch_mode(Mode::Fzf),
                    //Key::Char('k') => self.mode = Mode::Command,
                    //Key::Char('j') => self.mode = Mode::Command,
                    Key::Ctrl('u') => self.scroll_pos += 5,
                    Key::Ctrl('d') => self.scroll_pos = self.scroll_pos.checked_sub(5).unwrap_or(0),
                    _ => ()
                }
            },
            Mode::TextInput => {
                match key {
                    Key::Char('\n') => {
                        self.prev_mode();
                        self.process_text_input();
                    }
                    Key::Char(chr) => {
                        self.add_character(chr);
                    }
                    Key::Backspace => {
                        self.remove_character();
                    }
                    Key::Esc => {
                        self.prev_mode();
                        if let Mode::Command = self.mode { self.prev_mode(); }
                    },
                    _ => ()
                }
            },
            Mode::ChannelSelect => {
                match key {
                    Key::Esc => {self.mode = Mode::Normal}
                    Key::Char('\t') => {self.mode = Mode::ServerSelect}
                    Key::Char('k') => self.active_server().prev_channel(),
                    Key::Char('j') => self.active_server().next_channel(),
                    _ => ()
                }},
            Mode::ServerSelect => {
                match key {
                    Key::Esc => {self.mode = Mode::Normal}
                    Key::Char('\t') => {self.mode = Mode::ChannelSelect}
                    Key::Char('k') => self.prev_server(),
                    Key::Char('j') => self.next_server(),
                    _ => ()
                }
                },
            Mode::Command => {
                match key {
                    Key::Esc => {self.mode = Mode::Normal}
                    Key::Char('q') => self.quit(),
                    _ =>self.print(format!("Unsupported key for {:?} mode: {:?}", self.mode, key))
            }},
            _ => ()
        }
    }
    fn print(&self, what: String){
        self.to_provider.send(MsgToDiscord::Echo(what));
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
 
    let mut app_state = AppState::new(channel_to_discord.0.clone(),channel_from_discord.1.clone());
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
                Mode::TextInput => terminal.show_cursor(),
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
            Paragraph::default().text(&format!("Mode: {:?}, Scroll:{}", state.mode, state.scroll_pos)).render(t, &chunks[1]);
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
            let nm = (chunks[0].height as usize).checked_sub(2).unwrap_or(0);
            let left_bound = n.checked_sub(nm+state.scroll_pos).unwrap_or(0);
            
            ChatWidget::new(&msgs.to_vec())
				.scroll(state.scroll_pos)
                .block(Block::default().borders(Borders::ALL).title(&format!("#{}", channel_name)[..]))
                .render(t, &chunks[0]);
/*
            let msgs = msgs[left_bound..n].iter().map( |msg| {
                Item::StyledData(
                    format!("{}: {}", &msg.author.name[..], &msg.content[..]),
                    &style,
                )
            });


            List::new(msgs)
                .block(Block::default().borders(Borders::ALL).title(&format!("#{}", channel_name)[..]))
                .render(t, &chunks[0]);
*/
            match state.mode {
              
                Mode::TextInput => {
              let help = match state.mode_stack.last().unwrap() {
               &Mode::Command => String::from("Command"),
               &Mode::Normal => format!("Message #{}", channel_name),
               x => format!("Input for {:?}", x)};
                     Paragraph::default()
                        .text(&state.content[..])
                        .block(Block::default().borders(Borders::ALL).title(help.as_ref()))
                        .render(t, &chunks[1]);
                }
                _ => {
                    List::new(match state.mode {
                        Mode::Normal => vec!["c - Select Channel", "s - Select Server", "i - Insert Message", ": - Command"],
                        Mode::ChannelSelect => vec!["j/k - Move", "Tab - Select Server", "Enter - Accept"],
                        Mode::ServerSelect => vec!["j/k - Move", "Tab - Select Channel", "Enter - Accept"],
                        _ => vec![]
                    }.iter().map(|x| Item::Data(x)))
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
                .block(Block::default().borders(Borders::ALL).title("Servers").border_style(match state.mode {
                    Mode::ServerSelect => selectedBorder,
                    _ => defaultBorder
                }))
                .items(&state.servers)
                .select(state.active_server)
                .highlight_style(Style::default().fg(Color::Green).modifier(Modifier::Bold))
                .highlight_symbol(
                    match state.mode {
                    Mode::ServerSelect => ">",
                    _ => "-"
                })
                .render(t, &chunks[0]);

            SelectableList::default()
                .block(Block::default().borders(Borders::ALL).title("Channels").border_style(match state.mode {
                    Mode::ChannelSelect => selectedBorder,
                    _ => defaultBorder
                }))
                .items(&state.servers[state.active_server].channels)
                .select(state.servers[state.active_server].active_channel)
                .highlight_style(Style::default().fg(Color::Green).modifier(Modifier::Bold))
                .highlight_symbol(
                    match state.mode {
                    Mode::ChannelSelect => ">",
                    _ => "-"
                })
                .render(t, &chunks[1]);

        });
}
