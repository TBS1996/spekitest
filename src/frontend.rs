//! this will be about actually using the program like reviewing and all that

use std::io::stdout;

use crate::card::{Card, Side};
use crate::common::Category;
use crate::config::Config;
use crate::folders::review_card_in_directory;
use crate::git::git_save;

pub fn run(config: Config) {
    let menu_items = vec![
        "Add new cards",
        "Review cards",
        "Add unfinished cards",
        "Save progress",
        "Quit",
    ];

    loop {
        match draw_menu(&menu_items) {
            0 => add_cards(Category::default(), true),
            1 => review_card_in_directory(&Category::default()),
            2 => add_cards(Category::default(), false),
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
/*
fn print_cards(conn: &Conn, cards: &[Id]) {
    println!(
        "actions:
d: delete
e: edit

q: quit

to edit or delete, press first the number of the card then the action letter.
    "
    );

    for (index, id) in cards.iter().enumerate() {
        let question = Card::get_card_question(*id, conn);
        println!("{}: {}", index, question);
    }
}

pub fn view_cards(conn: &Conn, category: &Category) {
    let cards = folders::get_card_ids_from_category(category);
    print_cards(conn, &cards);

    let mut input = String::new();
    loop {
        input.clear();
        std::io::stdin().read_line(&mut input).unwrap();
        input.pop();
        if input == "q" {
            return;
        }

        let x = input.split_once(' ');
        if let Some(x) = x {
            if let (Ok(card), action) = (x.0.parse::<usize>(), x.1) {
                if cards.len() > card {
                    match action {
                        "d" => {
                            Card::delete_card(cards[card], conn);
                            let cards = folders::get_card_ids_from_category(category);
                            print_cards(conn, &cards);
                        }
                        "e" => Card::edit_card(cards[card], conn),
                        "q" => return,
                        _ => {
                            let _ = dbg!(&action);
                        }
                    }
                }
            }
        }
    }
}
*/
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

pub fn review_cards(cards: Vec<Card>, category: &Category) {
    let mut grade_given = String::new();
    let cardqty = cards.len();
    for (index, mut card) in cards.into_iter().enumerate() {
        println!("Review card");
        println!("{}", card.front.text);
        card.front.audio.play_audio();
        std::io::stdin().read_line(&mut String::new()).unwrap();
        println!("----------");
        println!("{}", card.back.text);
        card.back.audio.play_audio();

        loop {
            println!("Reviewing {}/{}", index, cardqty);
            grade_given.clear();
            std::io::stdin().read_line(&mut grade_given).unwrap();
            grade_given.pop();

            let grade = grade_given.parse();

            match grade {
                Ok(grade) => {
                    card.new_review(grade, category);
                }
                Err(_) => continue,
            }
            break;
        }
    }
    println!("Nothing left to review :D");
}

use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{read, Event, KeyCode},
    execute,
    style::{ResetColor, SetForegroundColor},
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
};

pub fn draw_message(message: &str) {
    enable_raw_mode().unwrap();

    let mut stdout = stdout();
    execute!(stdout, Hide).unwrap();

    execute!(stdout, Clear(ClearType::All)).unwrap();
    println!("{message}");
    execute!(stdout, ResetColor).unwrap();
    read().unwrap();
    execute!(stdout, Clear(ClearType::All)).unwrap();
    disable_raw_mode().unwrap();
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
