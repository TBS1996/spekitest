use std::io::Stdout;

use crossterm::cursor::MoveTo;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::execute;
use crossterm::terminal::Clear;
use crossterm::terminal::ClearType;

use crate::categories::Category;
use crate::frontend::NotOption;
use crate::frontend::Page;

use crate::card::{AnnoCard, Card};
use crate::config::Config;
use crate::folders::view_cards_in_explorer;
use crate::frontend::move_far_left;
use crate::git::git_save;

use crate::frontend::ItemPicker;

use super::ControlEnum;
use super::ControlRes;

#[derive(Default)]
pub struct CardAdder {
    category: NotOption<Category>,
    tags: Vec<String>,
    front: String,
    back: String,
    on_front_side: bool,
}

impl Default for NotOption<Category> {
    fn default() -> Self {
        let categories = Category::load_all().unwrap();
        let picker = ItemPicker::new(categories);
        Self::Picker(picker)
    }
}

impl CardAdder {
    pub fn new() -> Self {
        Self {
            on_front_side: true,
            ..Default::default()
        }
    }
}

impl CardAdder {
    fn edit_front(&mut self, key: KeyCode) {}
    fn edit_back(&mut self) {}
}

impl Page for CardAdder {
    fn view(&self, stdout: &mut Stdout) {
        match &self.category {
            NotOption::Some(_) => {
                execute!(stdout, Clear(ClearType::All)).unwrap();
                execute!(stdout, MoveTo(0, 0)).unwrap();
                println!("{}", self.front);
                move_far_left(stdout);
                println!("-----------------");
                move_far_left(stdout);
                println!("{}", self.back);
            }
            NotOption::Picker(picker) => {
                picker.view_with_formatter(stdout, Category::print_it_with_depth);
            }
        }
    }

    fn control(&mut self, key: KeyEvent) -> ControlRes {
        match &mut self.category {
            NotOption::Picker(picker) => match picker.control(key) {
                ControlEnum::Some(cat) => self.category = NotOption::Some(cat),
                ControlEnum::None => {}
                ControlEnum::Continue => {}
            },
            NotOption::Some(category) => match key.code {
                KeyCode::Char(c) if self.on_front_side => self.front.push(c),
                KeyCode::Char(c) if !self.on_front_side => self.back.push(c),
                KeyCode::Enter if self.on_front_side => self.on_front_side = false,
                KeyCode::Enter | KeyCode::BackTab if !self.on_front_side => {
                    let mut card = Card::new_simple(
                        std::mem::take(&mut self.front),
                        std::mem::take(&mut self.back),
                    );

                    if key.code == KeyCode::BackTab {
                        card.meta.finished = false;
                    }

                    card.meta.tags.extend(self.tags.clone());
                    card.save_new_card(category);
                }
                KeyCode::Backspace if self.on_front_side => {
                    let _ = self.front.pop();
                }
                KeyCode::Backspace if !self.on_front_side => {
                    let _ = self.back.pop();
                }
                _ => {}
            },
        };
        ControlRes::KeepGoing
    }
}
