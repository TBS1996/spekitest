//! this will be about actually using the program like reviewing and all that

use std::io::stdout;

use crate::card::{Card, Side};
use crate::common::Category;
use crate::config::Config;
use crate::folders::{get_pending_cards_from_category, get_review_cards_from_category};
use crate::git::git_save;

pub fn run(config: Config) {
    let menu_items = vec![
        "Add new cards",
        "Review cards",
        "Review pending cards",
        "Save progress",
        "Quit",
    ];

    loop {
        match draw_menu(&menu_items) {
            0 => add_cards(Category::default(), true),
            1 => review_cards(),
            2 => review_pending_cards(),
            3 => {
                println!("saving progress!");
                git_save(config.read_git_remote().is_some());
            }
            4 => {
                git_save(config.read_git_remote().is_some());
                return;
            }
            _ => {}
        };
    }
}

pub fn add_cards(category: Category, finished: bool) {
    let mut input = String::new();
    let mut front;
    let mut back;

    if finished {
        println!("Adding cards to: {}", category.joined());
    } else {
        println!("Adding unfinished cards to: {}", category.joined());
    }

    loop {
        input.clear();
        println!("Front side");
        std::io::stdin().read_line(&mut input).unwrap();
        input.pop();
        if input == "q" {
            return;
        }
        front = std::mem::take(&mut input);
        println!("Back side");
        std::io::stdin().read_line(&mut input).unwrap();
        input.pop();
        if input == "q" {
            return;
        }
        back = std::mem::take(&mut input);
        let mut card = Card {
            front: Side {
                text: front,
                ..Default::default()
            },
            back: Side {
                text: back,
                ..Default::default()
            },
            ..Default::default()
        };

        if !finished {
            card.meta.finished = false;
        }

        card.save_card(Some(category.clone()));
    }
}

pub fn review_pending_cards() {
    for category in Category::load_all().unwrap() {
        let cards = get_pending_cards_from_category(&category);
        if rev_cards(cards, &category) {
            return;
        }
    }
    draw_message("Nothing left to review!");
}

fn rev_cards(cards: Vec<Card>, category: &Category) -> bool {
    for (_, mut card) in cards.into_iter().enumerate() {
        let msg = "Review card".to_string();
        let msg = format!("{}\n\n{}", msg, card.front.text);
        draw_message(&msg);
        card.front.audio.play_audio();
        let msg = format!("{}\n\n{}", msg, "----------------");
        let msg = format!("{}\n\n{}", msg, card.back.text);
        std::io::stdin().read_line(&mut String::new()).unwrap();
        card.back.audio.play_audio();

        loop {
            let grade_given = draw_message(&msg);
            if grade_given == 'q' {
                return true;
            }

            match grade_given.to_string().parse() {
                Ok(grade) => {
                    card.new_review(grade, category);
                }
                Err(_) => continue,
            }
            break;
        }
    }
    false
}

pub fn review_cards() {
    for category in Category::load_all().unwrap() {
        let cards = get_review_cards_from_category(&category);
        if rev_cards(cards, &category) {
            return;
        }
    }
    draw_message("Nothing left to review!");
}

use crossterm::event::KeyEvent;
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{read, Event, KeyCode},
    execute,
    style::{ResetColor, SetForegroundColor},
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
};

pub fn draw_message(message: &str) -> char {
    enable_raw_mode().unwrap();

    let mut stdout = stdout();
    execute!(stdout, Hide).unwrap();
    execute!(stdout, Show, MoveTo(0, 0)).unwrap();

    execute!(stdout, Clear(ClearType::All)).unwrap();
    println!("{}", message);
    execute!(stdout, ResetColor).unwrap();

    let pressed_char = loop {
        match read().unwrap() {
            Event::Key(KeyEvent {
                code: KeyCode::Char(c),
                ..
            }) => break c,
            _ => {}
        }
    };

    execute!(stdout, Clear(ClearType::All)).unwrap();
    disable_raw_mode().unwrap();

    pressed_char
}

fn draw_menu(items: &[&str]) -> usize {
    enable_raw_mode().unwrap();

    let mut stdout = stdout();
    execute!(stdout, Hide).unwrap();

    let mut selected = 0;

    loop {
        execute!(stdout, Clear(ClearType::All)).unwrap();

        for (index, item) in items.iter().enumerate() {
            execute!(stdout, MoveTo(0, index as u16)).unwrap();

            if index == selected {
                execute!(stdout, SetForegroundColor(crossterm::style::Color::Blue)).unwrap();
                println!("> {}", item);
                execute!(stdout, ResetColor).unwrap();
            } else {
                println!("  {}", item);
            }
        }

        // Await input from user
        if let Event::Key(event) = read().unwrap() {
            match event.code {
                KeyCode::Up => {
                    selected = selected.saturating_sub(1);
                }
                KeyCode::Down => {
                    if selected < items.len() - 1 {
                        selected += 1;
                    }
                }
                KeyCode::Enter => {
                    execute!(stdout, Clear(ClearType::All)).unwrap();
                    execute!(stdout, Show, MoveTo(0, items.len() as u16 + 1)).unwrap();
                    disable_raw_mode().unwrap();
                    return selected;
                }
                _ => {}
            }
        }
    }
}
