use discord::Discord;
use discord::model::{Message, Event};
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
    ChatMsg(Message),
    ServerInfo(String),
    EchoResponse(String),
}
#[derive(Debug)]
pub enum MsgToDiscord {
    RequestServerInfo,
    Echo(String),
}
impl DiscordProvider {
    pub fn init(
        user_token: String,
        channel: (Sender<MsgFromDiscord>, Receiver<MsgToDiscord>),
    ) -> Self {
        DiscordProvider {
            discord: match Discord::from_user_token(&user_token) {
                Ok(x) => Some(x),
                Err(x) => {
                    println!("Couldn't log in! {:?}", x);
                    None
                }
            },
            tx: channel.0,
            rx: channel.1,
        }
    }
    pub fn outgoing_loop(self) {
        let mut connection = self.discord
            .expect("Man u gotta log in to discord first!")
            .connect()
            .expect("couldnt connect via websocket!")
            .0;

        let ifacec = chan::sync(0);
        let ifacetx = ifacec.0.clone();
        thread::spawn(move || loop {
            let evt = match connection.recv_event() {
                Ok(x) => x,
                Err(x) => {
                    continue;
                }
            };
            ifacetx.send(evt);
        });
        let rx = self.rx;
        let ifacerx = ifacec.1.clone();
        loop {
            chan_select! {
                default => {},
                rx.recv() -> val => {
                    let x = val.unwrap();
                    match x {
                        MsgToDiscord::RequestServerInfo => {

                        },
                        MsgToDiscord::Echo(s) => {
                            self.tx.send(MsgFromDiscord::EchoResponse(s));
                        }
                    }
                },
                ifacerx.recv() -> val => {
                    let evt = val.unwrap();
                    match evt {
                    Event::MessageCreate(msg) => {
                            self.tx.send(MsgFromDiscord::ChatMsg(msg));
                        },
                        _ => {}
                     }    
                }, 
            }
            thread::sleep_ms(50); // AAAAAAAAAAAAAAAAAAAUGH
        }
    }
}
