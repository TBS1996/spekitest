//! this will be about actually using the program like reviewing and all that

use crate::card::{Card, Side};
use crate::common::Category;
use crate::{folders, Conn, Id};

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

pub fn add_cards(conn: &Conn, category: Category) {
    let mut input = String::new();
    let mut front;
    let mut back;

    println!("Adding cards to: {}", category.joined());

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
        let card = Card {
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

        card.save_card(Some(category.clone()), conn);
    }
}

pub fn review_cards(conn: &Conn, cards: Vec<Card>, category: &Category) {
    let mut grade_given = String::new();
    let cardqty = cards.len();
    for (index, card) in cards.into_iter().enumerate() {
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
