use discord::Discord;
use discord::model::{Message};
use chan::{Sender,Receiver};

//#[derive(Debug)]
pub struct DiscordProvider {
    discord: Discord,
    tx: Sender<Msg>,
    rx: Receiver<Msg>

}
#[derive(Debug)]
pub enum Msg {
    ToDiscord(MsgToDiscord), FromDiscord(MsgFromDiscord)
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
    pub fn init(user_token: String, channel: (Sender<Msg>, Receiver<Msg>)) -> Self {
        DiscordProvider {
            discord: Discord::from_user_token(&user_token).expect("Unable to login"),
            tx: channel.0,
            rx: channel.1
        }
    }
    pub fn outgoing_loop(self) {
        loop {
           if let Msg::ToDiscord(x) = self.rx.recv().unwrap() {
                match x {
                    RequestServerInfo => {

                    },
                    MsgToDiscord::Echo(s) => {
                        self.tx.send(Msg::FromDiscord(MsgFromDiscord::EchoResponse(s)));
                    }
                } 
           }
        }
    }
}