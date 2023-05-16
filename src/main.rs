use folders::*;
use rusqlite::Result;

use std::io::{self};

use uuid::Uuid;

use crate::card::Card;
use crate::common::Category;

mod cache;
mod card;
mod common;
mod folders;
mod frontend;

type Conn = rusqlite::Connection;

#[cfg(test)]
pub const PATH: &str = "./testing/";
#[cfg(not(test))]
pub const PATH: &str = "./cards/";

type Id = Uuid;

fn main() -> Result<()> {
    let conn = cache::init()?;

    let menu_stuff = "Welcome! :D
choose action followed by directory .

actions: 
1. Add new card
2. Add new category
3. Review cards
...

5. index cards
6. view cards

actions:

";

    let mut input = String::new();

    loop {
        println!("{}", menu_stuff);
        let mut categories = Category::load_all().unwrap();
        categories.insert(0, Category(vec!["default".into()]));
        for (index, category) in categories.iter().enumerate() {
            println!("{}, {}", index, category.joined());
        }
        categories[0].0.clear();
        input.clear();
        io::stdin().read_line(&mut input).unwrap();
        input.pop();

        let (action, dir) = input.split_once(' ').unwrap_or_else(|| ("0", &input));
        let dir = match dir.parse::<usize>() {
            Ok(dir) => {
                if dir >= categories.len() {
                    0
                } else {
                    dir
                }
            }
            Err(_) => 0,
        };

        let category = &categories[dir];

        match action {
            "1" => frontend::add_cards(&conn, category.to_owned()),
            "2" => {
                let _ = create_category(category);
            }
            "3" => review_card_in_directory(&conn, category),
            "4" => return Ok(()),
            "5" => cache::index_cards(&conn),
            "6" => frontend::view_cards(&conn, category),
            _ => println!("Invalid input"),
        }
    }
}

/*
TODO list! :D








 */
