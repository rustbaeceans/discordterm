use discord::{Discord, Connection, GetMessages};
use discord::model::*;
use chan::{Sender, Receiver};
use chan;
use thread;
use std::cmp::min;
use std::fmt;

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

pub enum MsgToDiscord {
    GetServers,
    GetChannels(ServerId),
    GetMessages(ChannelId, GetMessages, usize),
    SendMessage(ChannelId, String),
    Logout, // FIN
    Echo(String), // Testing echo back what we got
}

impl fmt::Debug for MsgToDiscord {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use MsgToDiscord::*;
        match self {
            &GetMessages(x,_,z) => write!(f, "Get {} messages from {:?}", z, x),
            x => write!(f,"{:?}", x)
        }
    }
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
                    MsgToDiscord::GetMessages(id, gm, count) => (),
                    x => panic!("Unrecognized message {:?}",x)
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
struct MessageIterator<'a> {
    last: Option<MessageId>,
    client: &'a Discord,
    channelid: ChannelId,
    total_desired: usize,
}
impl<'a> MessageIterator<'a> {
    fn new(client: &'a Discord, channelid: ChannelId, count: usize) -> MessageIterator<'a> {
        MessageIterator {
            last: None,
            client,
            channelid,
            total_desired: count,
        }
    }
    /// Flattens and collects the iterator of Vecs into a single Vec
    fn collect(self) -> Vec<Message> {
        // self is moved in to consume the iterator into a list
        self.flat_map(|x| x).collect()
    }
}
impl<'a> Iterator for MessageIterator<'a> {
    type Item = Vec<Message>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.total_desired <= 0 {
            return None;
        }
        let what = match self.last {
            Some(id) => GetMessages::Before(id),
            None => GetMessages::MostRecent,
        };
        let limit = min(self.total_desired, 100); // API is limited to 100

        println!("Getting {} messages from {}", limit, self.channelid);
        let messages = self.client
            .get_messages(self.channelid, what, Some(limit as u64))
            .expect("Failed to get messages.");

        if messages.len() < limit {
           self.total_desired = 0; 
        } else {
            self.total_desired -= messages.len();
        }

        self.last = Some(messages.last().unwrap().id);

        Some(messages)
    }
}
