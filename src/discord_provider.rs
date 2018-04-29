use discord::Discord;
use discord::Connection;
use discord::model::*;
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
	Exit, // FIN-ACK basically
    EchoResponse(String)
}

#[derive(Debug)]
pub enum MsgToDiscord {
    GetServers,
    GetChannels(ServerId),
    SendMessage(ChannelId, String),
    Logout, // FIN
    Echo(String), // Testing echo back what we got
}

impl DiscordProvider {
    pub fn init(
        discord: Discord,
        channel: (Sender<MsgFromDiscord>, Receiver<MsgToDiscord>),
    ) -> Self {
        DiscordProvider {
            discord: Some(discord),
            tx: channel.0,
            rx: channel.1,
        }
    }

    pub fn start_provider(self) {
        let discord_client = self.discord
            .expect("Login to discord first!");
        let mut connection = discord_client
            .connect()
            .expect("Failed to initialize websocket connection")
            .0;

        let (sender, reciever) = chan::async();
		let (sdone, rdone) = chan::async();
        thread::spawn(move || monitor_websocket(connection, sender,rdone));
        
        handle_messages(discord_client, self.tx, self.rx, reciever, sdone)
    }
}

fn monitor_websocket(mut connection: Connection, discord_sender: Sender<Event>, close:Receiver<()>) {
    loop {
		chan_select! {
			default => (),
			close.recv() => {println!("Websocket closing."); break;},
            
		}
        let event = match connection.recv_event() {
            Ok(event) => discord_sender.send(event),
            Err(error) => {
                // Don't spam if something goes wrong
                thread::sleep_ms(1000);
                continue;
            }
        };

    };
}

// Handle messages to and from the main module
fn handle_messages(
    discord: Discord,
    ui_sender: Sender<MsgFromDiscord>,
    ui_reciever: Receiver<MsgToDiscord>,
    discord_reciever: Receiver<Event>,
	close:Sender<()>) {
    loop {
        chan_select! {
            default => {},
            ui_reciever.recv() -> val => {
                let message = val.unwrap();
                //println!("{:?}", message);
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
                    MsgToDiscord::Logout => {
						close.send(());
						ui_sender.send(MsgFromDiscord::Exit);
						return;	
                    }
                    MsgToDiscord::Echo(message) => {
                        ui_sender.send(MsgFromDiscord::EchoResponse(message));
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
