//! this will be about actually using the program like reviewing and all that

use std::io;

use crate::card::{Card, Side};
use crate::common::Category;
use crate::folders::review_card_in_directory;
use crate::{folders, git_save, Conn, Id};

pub fn main_loop(conn: &Conn) {
    let menu_stuff = "Welcome! :D

1. Add new cards
2. Review cards
3. Add unfinished cards
";

    loop {
        clear_screen();
        println!("{}", menu_stuff);

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        input.pop();

        match input.as_str() {
            "1" => add_cards(conn, Category::default(), true),
            "2" => review_card_in_directory(conn, &Category::default()),
            "3" => add_cards(&conn, Category::default(), false),
            "s" => {
                println!("saving progress!");
                git_save();
            }
            "q" => {
                git_save();
                return;
            }
            _ => {
                println!("Invalid input!");
                println!();
                println!();
                continue;
            }
        };
    }
}

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

pub fn add_cards(conn: &Conn, category: Category, finished: bool) {
    let mut input = String::new();
    let mut front;
    let mut back;

    if finished {
        println!("Adding cards to: {}", category.joined());
    } else {
        println!("Adding unfinished cards to: {}", category.joined());
    }

    loop {
        clear_screen();
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

        card.save_card(Some(category.clone()), conn);
    }
}

use console::Term;

fn clear_screen() {
    let term = Term::stdout();
    term.clear_screen().unwrap();
}

pub fn review_cards(_conn: &Conn, cards: Vec<Card>, category: &Category) {
    let mut grade_given = String::new();
    let cardqty = cards.len();
    for (index, card) in cards.into_iter().enumerate() {
        clear_screen();
        let strength = card.calculate_strength();
        if card.calculate_strength() > 0.9 {
            println!(
                "Skipping: {}, reason: strength too high: {}",
                card.front.text, strength
            );
            continue;
        }

        println!("Review card");
        println!("{}", card.front.text);
        std::io::stdin().read_line(&mut String::new()).unwrap();
        println!("----------");
        println!("{}", card.back.text);

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
