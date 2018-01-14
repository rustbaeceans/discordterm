use discord::Discord;
use discord::model::{Message};
use chan::{Sender,Receiver};

//#[derive(Debug)]
pub struct DiscordProvider {
    discord: Option<Discord>,
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
           if let Msg::ToDiscord(x) = self.rx.recv().unwrap() {
                println!("Received message: {:?}", x);
                match x {
                    MsgToDiscord::RequestServerInfo => {

                    },
                    MsgToDiscord::Echo(s) => {
                        println!("Sending response");
                        self.tx.send(Msg::FromDiscord(MsgFromDiscord::EchoResponse(s)));
                    }
                } 
           }
        }
    }
}