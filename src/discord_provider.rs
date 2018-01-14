use discord::Discord;
use discord::model::{Message};
use chan::{Sender,Receiver};

type TX = Sender<MsgFromDiscord>;
type RX = Receiver<MsgToDiscord>;
//#[derive(Debug)]
struct DiscordProvider {
    discord: Discord,
    tx: TX,
    rx: RX

}
enum MsgFromDiscord {
    ChatMsg(Message),
    ServerInfo(String),
    EchoResponse(String)
}
enum MsgToDiscord {
    RequestServerInfo,
    Echo(String)
}
impl DiscordProvider {
    fn init(self, user_token: String, channel: (TX, RX)) -> Self {
        DiscordProvider {
            discord: Discord::from_user_token(&user_token).expect("Unable to login"),
            tx: channel.0,
            rx: channel.1
        }
    }
    fn loop(self) {
        loop {
           match self.rx.recv().unwrap() {
               RequestServerInfo => {

               },
               Echo(s) => {
                   self.tx.send(EchoResponse(s));
               }
           } 
        }
    }
}