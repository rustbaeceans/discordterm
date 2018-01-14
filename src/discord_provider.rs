use discord::Discord;
use discord::Connection;
use discord::model::*;
use discord::State;
use chan::{Sender, Receiver};
use chan;
use thread;

//#[derive(Debug)]
pub struct DiscordProvider {
    discord: Option<Discord>,
    tx: Sender<MsgFromDiscord>,
    rx: Receiver<MsgToDiscord>,
}

#[derive(Debug)]
pub enum MsgFromDiscord {
    Servers(Vec<ServerInfo>),
    Channels(ServerId, Vec<PublicChannel>),
    ChatMsg(Message),
    EchoResponse(String),
    Friends(Vec<Presence>),
}

#[derive(Debug)]
pub enum MsgToDiscord {
    GetServers,
    GetChannels(ServerId),
    SendMessage(ChannelId, String),
    Echo(String), // Testing echo back what we got
    GetFriends,
}

impl DiscordProvider {
    pub fn init(
        user_token: String,
        channel: (Sender<MsgFromDiscord>, Receiver<MsgToDiscord>),
    ) -> Self {
        DiscordProvider {
            discord: match Discord::from_user_token(&user_token) {
                Ok(discord_client) => Some(discord_client),
                Err(error) => {
                    panic!("Login Failed: {}", error);
                    None
                }
            },
            tx: channel.0,
            rx: channel.1,
        }
    }

    pub fn start_provider(self) {
        let discord_client = self.discord
            .expect("Login to discord first!");
        let (mut connection, ready) = discord_client
            .connect()
            .expect("Failed to initialize websocket connection");
        let (sender, reciever) = chan::sync(0);
        thread::spawn(move || monitor_websocket(connection, sender));

        let mut state = State::new(ready);        
        handle_messages(discord_client, state, self.tx, self.rx, reciever)
    }
}

fn monitor_websocket(mut connection: Connection, discord_sender: Sender<Event>) {
    loop {
        let event = match connection.recv_event() {
            Ok(event) => event,
            Err(error) => {
                // Don't spam if something goes wrong
                thread::sleep_ms(1000);
                continue;
            }
        };

        discord_sender.send(event);
    };
}

// Handle messages to and from the main module
fn handle_messages(
    discord: Discord,
    state: State,
    ui_sender: Sender<MsgFromDiscord>,
    ui_reciever: Receiver<MsgToDiscord>,
    discord_reciever: Receiver<Event>) {
    loop {
        chan_select! {
            default => {},
            ui_reciever.recv() -> val => {
                let message = val.unwrap();
                match message {
                    MsgToDiscord::GetServers => {
                        let s = discord.get_servers();
                        if let Ok(servers) = s {
                            ui_sender.send(MsgFromDiscord::Servers(servers));
                        }
                    },
                    MsgToDiscord::GetChannels(server_id) => {
                        let c = discord.get_server_channels(server_id);
                        if let Ok(channels) = c {
                            ui_sender.send(MsgFromDiscord::Channels(server_id, channels));
                        }
                    }
                    MsgToDiscord::SendMessage(channel, content) => {
                        discord.send_message(channel, &content, "", false);
                    },
                    MsgToDiscord::Echo(message) => {
                        ui_sender.send(MsgFromDiscord::EchoResponse(message));
                    },
                    MsgToDiscord::GetFriends => {
                        let friends = state.presences();
                        ui_sender.send(MsgFromDiscord::Friends((*friends).to_vec()));
                    },
                    _ => (),
                }
            },
            discord_reciever.recv() -> val => {
                let event = val.unwrap();
                match event {
                    Event::MessageCreate(msg) => {
                        ui_sender.send(MsgFromDiscord::ChatMsg(msg));
                    },
                    _ => {}
                }
            },
        }
    }
}