//! this will be about actually using the program like reviewing and all that

use std::fmt::Display;
use std::io::{stdout, Stdout};

use crate::card::{AnnoCard, Card};
use crate::categories::Category;
use crate::config::Config;
use crate::folders::view_cards_in_explorer;
use crate::git::git_save;

pub fn run() {
    enable_raw_mode().unwrap();
    let mut stdout = stdout();
    execute!(stdout, Hide).unwrap();

    let menu_items = vec![
        "Add new cards",
        "Review cards",
        "View cards",
        "Settings",
        "Debug",
        "search",
    ];

    while let Some(choice) = draw_menu(&mut stdout, menu_items.clone(), true) {
        match choice {
            0 => {
                let category = match choose_folder(&mut stdout) {
                    Some(category) => category,
                    None => continue,
                };

                add_cards(&mut stdout, category);
                let has_remote = Config::load().unwrap().git_remote.is_some();
                let _ = std::thread::spawn(move || git_save(has_remote));
            }
            1 => {
                let revtype =
                    match draw_menu(&mut stdout, vec!["Normal", "Pending", "Unfinished"], true) {
                        Some(x) => x,
                        None => continue,
                    };

                let category = match choose_folder(&mut stdout) {
                    Some(category) => category,
                    None => continue,
                };

                match revtype {
                    0 => {
                        review_cards(&mut stdout, category.clone());
                        draw_message(&mut stdout, "now reviewing pending cards");
                        review_pending_cards(&mut stdout, category.clone());
                        draw_message(&mut stdout, "Now reviewing unfinished cards");
                        review_unfinished_cards(&mut stdout, category.clone());
                    }
                    1 => {
                        review_pending_cards(&mut stdout, category.clone());
                        draw_message(&mut stdout, "Now reviewing unfinished cards");
                        review_unfinished_cards(&mut stdout, category.clone());
                    }
                    2 => review_unfinished_cards(&mut stdout, category),
                    _ => continue,
                }

                let has_remote = Config::load().unwrap().git_remote.is_some();
                let _ = std::thread::spawn(move || git_save(has_remote));
            }
            2 => view_cards_in_explorer(),
            3 => {
                let _ = Config::edit_with_vim();
            }
            4 => {
                let card = view_last_modified_cards(&mut stdout);
                if let Some(card) = card {
                    card.edit_with_vim();
                }
            }
            5 => {
                let input = match read_user_input(&mut stdout) {
                    Some(input) => input,
                    None => continue,
                };
                let card = view_search_cards(&mut stdout, input.0);
                if let Some(card) = card {
                    card.edit_with_vim();
                }
            }
            _ => {}
        };
    }
    execute!(stdout, Clear(ClearType::All)).unwrap();
    execute!(stdout, Show).unwrap();
    disable_raw_mode().unwrap();
}

use std::io::Write;

pub fn read_user_input(stdout: &mut Stdout) -> Option<(String, KeyCode)> {
    let mut input = String::new();
    let mut key_code;

    loop {
        if let Event::Key(event) = read().unwrap() {
            key_code = event.code;
            match event.code {
                KeyCode::Char(c) => {
                    input.push(c);
                    // You can decide whether to echo the input to the screen or not
                    print!("{}", c);
                    stdout.flush().unwrap(); // Make sure the char is displayed
                }
                KeyCode::Backspace if !input.is_empty() => {
                    input.pop();
                    let (x, y) = cursor::position().unwrap();

                    if x == 0 && y != 0 {
                        let (width, _) = terminal::size().unwrap();
                        execute!(stdout, MoveTo(width, y - 1), Print(" "),).unwrap();
                    } else {
                        execute!(stdout, MoveLeft(1), Print(" "), MoveLeft(1),).unwrap();
                    }
                    stdout.flush().unwrap();
                }
                KeyCode::Enter => break,
                KeyCode::Tab => break,
                KeyCode::Esc => return None,
                KeyCode::F(1) => break,
                _ => {}
            }
        }
    }
    Some((input, key_code))
}

fn move_far_left(stdout: &mut Stdout) {
    let (_, y) = cursor::position().unwrap();
    execute!(stdout, MoveTo(0, y)).unwrap();
}

fn update_status_bar(stdout: &mut Stdout, msg: &str) {
    let pre_pos = cursor::position().unwrap();
    execute!(stdout, MoveTo(0, 0)).unwrap();
    writeln!(stdout, "{}", msg).unwrap();
    stdout.flush().unwrap();
    execute!(stdout, cursor::MoveTo(pre_pos.0, pre_pos.1)).unwrap();
}

pub fn add_card(stdout: &mut Stdout, category: Category) -> Option<AnnoCard> {
    execute!(stdout, Clear(ClearType::All)).unwrap();
    execute!(stdout, MoveTo(0, 1)).unwrap();
    update_status_bar(stdout, "--front side--");
    let mut key_code;

    let (front_text, code) = match read_user_input(stdout) {
        Some((text, code)) => (text, code),
        None => return None,
    };

    key_code = code;

    let back_text = if key_code != KeyCode::Tab {
        execute!(stdout, MoveDown(2)).unwrap();
        move_far_left(stdout);
        println!("--back side--");
        move_far_left(stdout);

        let (back_text, code) = match read_user_input(stdout) {
            Some((text, code)) => (text, code),
            None => return None,
        };

        key_code = code;

        back_text
    } else {
        String::new()
    };

    let mut card = Card::new_simple(front_text, back_text);

    if key_code == KeyCode::Tab {
        card.meta.finished = false;
    }

    Some(card.save_new_card(&category))
}

pub fn add_cards(stdout: &mut Stdout, category: Category) {
    loop {
        if add_card(stdout, category.clone()).is_none() {
            return;
        }
    }
}

pub fn review_unfinished_cards(stdout: &mut Stdout, category: Category) {
    let mut cards = category.get_unfinished_cards(true);
    let mut selected = 0;

    loop {
        if cards.is_empty() {
            break;
        }
        let cardqty = cards.len();
        let mut card = &mut cards[selected];
        let get_message = |card: &AnnoCard| {
            format!(
                "{}   {}/{}\n{}\n-------------------\n{}",
                category.print_full(),
                selected + 1,
                cardqty,
                card.0.front.text,
                card.0.back.text
            )
        };

        match draw_message(stdout, &get_message(card)) {
            KeyCode::Char('f') => {
                card.0.meta.finished = true;
                cards.remove(selected);
                selected = selected.saturating_sub(1);
            }
            KeyCode::Char('y') => {
                *card = match pick_card_from_search(stdout) {
                    Some(chosen_card) => {
                        card.0.meta.dependencies.push(chosen_card.0.meta.id);
                        card.update_card()
                    }
                    None => continue,
                }
            }
            KeyCode::Char('t') => {
                *card = match pick_card_from_search(stdout) {
                    Some(chosen_card) => {
                        card.0.meta.dependents.push(chosen_card.0.meta.id);
                        card.update_card()
                    }
                    None => continue,
                }
            }
            KeyCode::Char('T') => {
                draw_message(stdout, "Adding new dependent");
                if let Some(updated_card) =
                    add_dependent(stdout, card.to_owned(), Some(category.clone()))
                {
                    *card = updated_card;
                }
            }
            KeyCode::Char('Y') => {
                draw_message(stdout, "Adding new dependency");
                if let Some(updated_card) =
                    add_dependency(stdout, card.to_owned(), Some(category.clone()))
                {
                    *card = updated_card;
                }
            }
            KeyCode::Char('e') => {
                *card = card.edit_with_vim();
            }
            KeyCode::Right => {
                if selected != cards.len() - 1 {
                    selected += 1;
                }
            }
            KeyCode::Left => selected = selected.saturating_sub(1),

            key if should_exit(&key) => break,
            _ => {}
        }
    }
    draw_message(stdout, "Nothing left to review!");
}

pub fn review_pending_cards(stdout: &mut Stdout, category: Category) {
    let categories = category.get_following_categories();

    for category in categories {
        let cards = category.get_pending_cards();
        if rev_cards(stdout, cards, &category) {
            return;
        }
    }
}

fn update_card_review_status(stdout: &mut Stdout, i: usize, qty: usize, category: &Category) {
    let msg = format!(
        "Reviewing card {}/{} in {}",
        i + 1,
        qty,
        category.print_it()
    );
    update_status_bar(stdout, &msg);
}

fn print_card_review_front(stdout: &mut Stdout, card: &mut Card, sound: bool) {
    execute!(stdout, MoveTo(0, 1)).unwrap();
    println!("{}", card.front.text);
    if sound {
        card.front.audio.play_audio();
    }
}

fn print_card_review_back(stdout: &mut Stdout, card: &mut Card, sound: bool) {
    move_far_left(stdout);
    execute!(stdout, MoveDown(1)).unwrap();
    move_far_left(stdout);
    println!("------------------");
    execute!(stdout, MoveDown(1)).unwrap();
    move_far_left(stdout);
    println!("{}", card.back.text);
    move_far_left(stdout);

    if sound {
        card.back.audio.play_audio();
    }
}

fn should_exit(key: &KeyCode) -> bool {
    matches!(key, KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q'))
}

fn print_card_review_full(stdout: &mut Stdout, card: &mut Card) {
    execute!(stdout, Clear(ClearType::All)).unwrap();
    print_card_review_front(stdout, card, false);
    print_card_review_back(stdout, card, false);
}

fn rev_cards(stdout: &mut Stdout, mut cards: Vec<AnnoCard>, category: &Category) -> bool {
    let qty = cards.len();

    for (i, card) in cards.iter_mut().enumerate() {
        execute!(stdout, Clear(ClearType::All)).unwrap();
        update_card_review_status(stdout, i, qty, category);
        print_card_review_front(stdout, card.card_as_mut_ref(), true);

        if should_exit(&get_keycode()) {
            return false;
        }

        print_card_review_back(stdout, card.card_as_mut_ref(), true);
        loop {
            match get_char() {
                'q' => return false,
                'e' => {
                    *card = card.edit_with_vim();
                    print_card_review_full(stdout, card.card_as_mut_ref());
                }
                'j' => {
                    card.0.meta.suspended = true;
                    card.update_card();
                    draw_message(stdout, "card suspended");
                    break;
                }
                // skip card
                's' => break,

                c => match c.to_string().parse() {
                    Ok(grade) => {
                        *card = card.new_review(grade);
                        break;
                    }
                    _ => continue,
                },
            }
        }
    }
    false
}

pub fn review_cards(stdout: &mut Stdout, category: Category) {
    let categories = category.get_following_categories();

    for category in categories {
        let cards = category.get_review_cards();
        if rev_cards(stdout, cards, &category) {
            return;
        }
    }
}

use crossterm::cursor::{self, MoveDown, MoveLeft, Show};
use crossterm::event::KeyEvent;
use crossterm::style::Print;
use crossterm::terminal;
use crossterm::{
    cursor::{Hide, MoveTo},
    event::{read, Event, KeyCode},
    execute,
    style::{ResetColor, SetForegroundColor},
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
};

pub fn get_keycode() -> KeyCode {
    loop {
        match read().unwrap() {
            Event::Key(KeyEvent { code, .. }) => return code,
            _ => continue,
        }
    }
}

pub fn get_char() -> char {
    loop {
        if let Ok(Event::Key(KeyEvent {
            code: KeyCode::Char(c),
            ..
        })) = read()
        {
            return c;
        }
    }
}

pub fn draw_message(stdout: &mut Stdout, message: &str) -> KeyCode {
    execute!(stdout, MoveTo(0, 0)).unwrap();

    execute!(stdout, Clear(ClearType::All)).unwrap();
    for char in message.chars() {
        print!("{char}");
        if char == '\n' {
            move_far_left(stdout);
        }
    }
    //println!("{}", message);
    execute!(stdout, ResetColor).unwrap();

    let pressed_char = get_keycode();

    execute!(stdout, Clear(ClearType::All)).unwrap();

    pressed_char
}

fn choose_folder(stdout: &mut Stdout) -> Option<Category> {
    pick_item_with_formatter(
        stdout,
        &Category::load_all().unwrap(),
        Category::print_it_with_depth,
    )
    .cloned()
}

fn pick_item<'a, T: Display>(stdout: &mut Stdout, items: &'a Vec<T>) -> Option<&'a T> {
    let formatter = |item: &T| format!("{}", item);
    pick_item_with_formatter(stdout, items, formatter)
}

fn pick_item_with_formatter<'a, T, F>(
    stdout: &mut Stdout,
    items: &'a Vec<T>,
    formatter: F,
) -> Option<&'a T>
where
    F: Fn(&T) -> String,
{
    let mut selected = 0;

    loop {
        execute!(stdout, Clear(ClearType::All)).unwrap();

        for (index, item) in items.iter().enumerate() {
            execute!(stdout, MoveTo(0, index as u16)).unwrap();

            if index == selected {
                execute!(stdout, SetForegroundColor(crossterm::style::Color::Blue)).unwrap();
                println!("> {}", formatter(item));
                execute!(stdout, ResetColor).unwrap();
            } else {
                println!("  {}", formatter(item));
            }
        }

        // Await input from user
        if let Event::Key(event) = read().unwrap() {
            match event.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    selected = selected.saturating_sub(1);
                }
                KeyCode::Char('G') => selected = items.len() - 1,
                KeyCode::Down | KeyCode::Char('j') => {
                    if selected < items.len() - 1 {
                        selected += 1;
                    }
                }
                KeyCode::Enter | KeyCode::Char(' ') => return Some(&items[selected]),
                key if should_exit(&key) => return None,
                _ => {}
            }
        }
    }
}

fn draw_menu(stdout: &mut Stdout, items: Vec<&str>, optional: bool) -> Option<usize> {
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
                KeyCode::Up | KeyCode::Char('k') => {
                    selected = selected.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if selected < items.len() - 1 {
                        selected += 1;
                    }
                }
                KeyCode::Char('G') => selected = items.len() - 1,
                KeyCode::Enter | KeyCode::Char(' ') => {
                    execute!(stdout, Clear(ClearType::All)).unwrap();
                    execute!(stdout, MoveTo(0, items.len() as u16 + 1)).unwrap();
                    return Some(selected);
                }
                KeyCode::Char('q') | KeyCode::Esc if optional => return None,
                _ => {}
            }
        }
    }
}

pub fn view_last_modified_cards(stdout: &mut Stdout) -> Option<AnnoCard> {
    let mut cards = AnnoCard::load_all();
    AnnoCard::sort_by_last_modified(&mut cards);
    cards.truncate(10);
    pick_item(stdout, &cards).cloned()
}

pub fn view_search_cards(stdout: &mut Stdout, searchterm: String) -> Option<AnnoCard> {
    let mut cards = AnnoCard::search(searchterm);
    cards.truncate(10);
    pick_item(stdout, &cards).cloned()
}

pub fn add_dependency(
    stdout: &mut Stdout,
    mut card: AnnoCard,
    category: Option<Category>,
) -> Option<AnnoCard> {
    let category = category.unwrap_or_else(|| card.1.category.clone());
    let new_dependency = add_card(stdout, category)?;
    card.0.meta.dependencies.push(new_dependency.0.meta.id);
    Some(card.update_card())
}

pub fn add_dependent(
    stdout: &mut Stdout,
    mut card: AnnoCard,
    category: Option<Category>,
) -> Option<AnnoCard> {
    let category = category.unwrap_or_else(|| card.1.category.clone());
    let new_dependent = add_card(stdout, category)?;
    card.0.meta.dependents.push(new_dependent.0.meta.id);
    Some(card.update_card())
}

pub fn pick_card_from_search(stdout: &mut Stdout) -> Option<AnnoCard> {
    let input = read_user_input(stdout)?;
    let cards = AnnoCard::search(input.0);
    pick_item(stdout, &cards).cloned()
}
