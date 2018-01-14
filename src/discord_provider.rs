use discord::Discord;
use discord::model::{Message};
use chan::{Sender,Receiver};
use thread;

//#[derive(Debug)]
pub struct DiscordProvider {
    discord: Option<Discord>,
    tx: Sender<MsgFromDiscord>,
    rx: Receiver<MsgToDiscord>

}

#[derive(Debug)]
pub enum MsgFromDiscord {
    ChatMsg(Message),
    ServerInfo(String),
    EchoResponse(String)
}
#[derive(Debug)]
pub enum MsgToDiscord {
    RequestServerInfo,
    Echo(String)
}
impl DiscordProvider {
    pub fn init(user_token: String, channel: (Sender<MsgFromDiscord>, Receiver<MsgToDiscord>)) -> Self {
        DiscordProvider {
            discord: match Discord::from_user_token(&user_token){
                Ok(x) => Some(x),
                Err(x) => { println!("Couldn't log in! {:?}",x); None }
            },
            tx: channel.0,
            rx: channel.1
        }
    }
    pub fn outgoing_loop(self) {
        loop {
            let x = self.rx.recv().unwrap();
            match x {
                MsgToDiscord::RequestServerInfo => {

                },
                MsgToDiscord::Echo(s) => {
                    self.tx.send(MsgFromDiscord::EchoResponse(s));
                }
            }
           thread::sleep_ms(50); // AAAAAAAAAAAAAAAAAAAUGH
        }
    }
}