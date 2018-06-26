use itertools::Itertools;
use discord;

use tui::layout::{Group, Size, Rect, Direction};
use tui::widgets::{Widget, Item, Block};
use tui::buffer::Buffer;
use tui::style::{Color, Modifier, Style};

use std::cmp::{max, min};

static style: Style = Style {
    fg: Color::Gray,
    bg: Color::Reset,
    modifier: Modifier::Reset
};

pub struct ChatWidget<'a>{
    selected: Option<usize>,
    scroll: usize,
    messages: &'a Vec<discord::model::Message>,
	 block: Option<Block<'a>>,
}

impl<'a> ChatWidget<'a> {
    pub fn new(messages: &'a Vec<discord::model::Message>) -> Self {
        ChatWidget {
            selected: None,
			block: None,
            scroll: 0,
            messages
        }
    }
	pub fn block(&'a mut self, block: Block<'a>) -> &mut Self {
		self.block = Some(block);
		self
	}
	pub fn scroll(&mut self, scroll:usize) -> &mut Self {
		self.scroll = scroll;
		self
	}
}

impl<'a> Widget for ChatWidget<'a> {
		fn draw(&mut self, area: &Rect, buf: &mut Buffer) {
		let list_area = match self.block {
            Some(ref mut b) => {
                b.draw(area, buf);
                b.inner(area)
            }
            None => *area,
        };

        if list_area.width < 1 || list_area.height < 1 {
            return;
        }

        self.background(&list_area, buf, Style::default().bg); 
        let n = self.messages.len();
        let nm = (list_area.height as usize).checked_sub(2).unwrap_or(0);
        let left_bound = n.checked_sub(nm+self.scroll).unwrap_or(0);
        let right_bound = min(n, nm.checked_sub(self.scroll).unwrap_or(0));
        let msgs = &self.messages[left_bound..right_bound];
        
        let mut y: usize = 0;

        msgs.iter().foreach( |msg| {
            let mut i: usize = 0;
            let content = format!("{}: {}", &msg.author.name[..], &msg.content[..]);
            while i < content.len(){
                let w = min(list_area.width as usize, content.len()-i);
                //println!("i={}, w={}, len={}, y={}", i, w, content.len(), y);
                //:
                buf.set_stringn(
                    list_area.left(),
                    list_area.top() + min(y,nm) as u16,
                    &content[i..i+w],
                    list_area.width as usize,
                    &Style::default(),
                );
                i += w;
                y += 1;
            }
        });
    }
}
