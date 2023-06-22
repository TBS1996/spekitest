//! this will be about actually using the program like reviewing and all that

use std::fmt::Display;
use std::io::{stdout, Stdout};

use crate::card::{AnnoCard, Card, CardAndRecall, ReviewType};
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
        "by tag",
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
                        review_cards(
                            &mut stdout,
                            category.clone(),
                            Box::new(Category::get_review_cards),
                        );
                        draw_message(&mut stdout, "now reviewing pending cards");
                        review_cards(
                            &mut stdout,
                            category.clone(),
                            Box::new(Category::get_pending_cards),
                        );
                        draw_message(&mut stdout, "Now reviewing unfinished cards");
                        review_cards(
                            &mut stdout,
                            category.clone(),
                            Box::new(Category::get_unfinished_cards),
                        );
                    }
                    1 => {
                        review_cards(
                            &mut stdout,
                            category.clone(),
                            Box::new(Category::get_pending_cards),
                        );
                        draw_message(&mut stdout, "Now reviewing unfinished cards");
                        review_cards(
                            &mut stdout,
                            category.clone(),
                            Box::new(Category::get_unfinished_cards),
                        );
                    }
                    2 => {
                        let cards = get_following_unfinished_cards(&category);
                        view_cards(&mut stdout, cards);
                        continue;
                        review_cards(
                            &mut stdout,
                            category.clone(),
                            Box::new(Category::get_unfinished_cards),
                        );
                    }
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
            6 => {
                let tags: Vec<String> = Category::get_all_tags().into_iter().collect();
                let tag = pick_item(&mut stdout, &tags);
                if let Some(tag) = tag {
                    let cards = AnnoCard::load_all()
                        .into_iter()
                        .filter(|card| card.0.meta.tags.contains(tag))
                        .collect();
                    view_cards(&mut stdout, cards);
                }
            }
            7 => {
                let mut cnt = 0;
                let mut cards = AnnoCard::print_by_strength();
                while let Some(card) = cards.pop_first() {
                    println!("{}", card);
                    move_far_left(&mut stdout);
                    cnt += 1;
                    if cnt == 50 {
                        break;
                    }
                }
                read().unwrap();
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

pub fn add_card(stdout: &mut Stdout, category: &Category) -> Option<AnnoCard> {
    execute!(stdout, Clear(ClearType::All)).unwrap();
    execute!(stdout, MoveTo(0, 0)).unwrap();
    let msg = format!("{}\n\t--front side--", category.print_full());

    write_string(stdout, &msg);
    execute!(stdout, MoveTo(0, 2)).unwrap();
    let mut key_code;

    let (front_text, code) = match read_user_input(stdout) {
        Some((text, code)) => (text, code),
        None => return None,
    };

    key_code = code;

    let back_text = if key_code != KeyCode::Tab {
        execute!(stdout, MoveDown(2)).unwrap();
        move_far_left(stdout);
        println!("\t--back side--");
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

    Some(card.save_new_card(category))
}

pub fn add_cards(stdout: &mut Stdout, category: Category) {
    loop {
        if add_card(stdout, &category).is_none() {
            return;
        }
    }
}

pub enum SomeStatus {
    Continue,
    Break,
}

fn review_unfinished_card(stdout: &mut Stdout, card: &mut AnnoCard) -> SomeStatus {
    let get_message = |card: &AnnoCard| {
        format!(
            "{}\n-------------------\n{}",
            card.0.front.text, card.0.back.text
        )
    };

    loop {
        match draw_message(stdout, &get_message(card)) {
            KeyCode::Char('f') => {
                card.0.meta.finished = true;
                card.update_card();
            }
            KeyCode::Char('s') => break,
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
            KeyCode::Char('g') => {
                let tags = card.1.category.get_tags().into_iter().collect();
                let tag = match pick_item(stdout, &tags) {
                    Some(tag) => tag,
                    None => continue,
                };
                card.0.meta.tags.insert(tag.to_owned());
                *card = card.update_card();
            }

            KeyCode::Char('T') => {
                draw_message(stdout, "Adding new dependent");
                if let Some(updated_card) =
                    add_dependent(stdout, card.to_owned(), Some(&card.1.category))
                {
                    *card = updated_card;
                }
            }
            KeyCode::Char('Y') => {
                draw_message(stdout, "Adding new dependency");
                if let Some(updated_card) =
                    add_dependency(stdout, card.to_owned(), Some(&card.1.category))
                {
                    *card = updated_card;
                }
            }
            KeyCode::Char('D') => {
                card.clone().delete();
                draw_message(stdout, "Card deleted");
                break;
            }
            KeyCode::Char('e') => {
                *card = card.edit_with_vim();
            }

            key if should_exit(&key) => return SomeStatus::Break,
            _ => {}
        };
    }
    SomeStatus::Continue
}

pub fn review_unfinished_cards(stdout: &mut Stdout, category: Category) {
    let mut cards = category.get_unfinished_cards();
    cards.reverse();
    let mut selected = 0;

    loop {
        if cards.is_empty() {
            break;
        }
        let cardqty = cards.len();
        let mut card = &mut cards[selected];
        let get_message = |card: &AnnoCard| {
            format!(
                "{}/{}   {}\n{}\n-------------------\n{}",
                selected + 1,
                cardqty,
                &card.1.category.print_full(),
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
            KeyCode::Char('s') => {
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
            KeyCode::Char('g') => {
                let tags = card.1.category.get_tags().into_iter().collect();
                let tag = match pick_item(stdout, &tags) {
                    Some(tag) => tag,
                    None => continue,
                };
                card.0.meta.tags.insert(tag.to_owned());
                *card = card.update_card();
            }

            KeyCode::Char('T') => {
                draw_message(stdout, "Adding new dependent");
                if let Some(updated_card) =
                    add_dependent(stdout, card.to_owned(), Some(&category.clone()))
                {
                    *card = updated_card;
                }
            }
            KeyCode::Char('Y') => {
                draw_message(stdout, "Adding new dependency");
                if let Some(updated_card) =
                    add_dependency(stdout, card.to_owned(), Some(&category.clone()))
                {
                    *card = updated_card;
                }
            }
            KeyCode::Char('D') => {
                let the_card = cards.remove(selected);
                the_card.delete();
                selected = selected.saturating_sub(1);
                draw_message(stdout, "Card deleted");
            }
            KeyCode::Char('e') => {
                *card = card.edit_with_vim();
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if selected != cards.len() - 1 {
                    selected += 1;
                }
            }
            KeyCode::Left | KeyCode::Char('h') => selected = selected.saturating_sub(1),

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
        if rev_cards(stdout, cards) {
            return;
        }
    }
}

fn update_card_review_status(
    stdout: &mut Stdout,
    i: usize,
    qty: usize,
    category: &Category,
) -> String {
    format!(
        "Reviewing card {}/{} in {}",
        i + 1,
        qty,
        category.print_it()
    )
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

fn review_card(stdout: &mut Stdout, card: &mut AnnoCard, status: String) -> SomeStatus {
    execute!(stdout, Clear(ClearType::All)).unwrap();
    update_status_bar(stdout, &status);

    print_card_review_front(stdout, card.card_as_mut_ref(), true);

    if should_exit(&get_keycode()) {
        return SomeStatus::Break;
    }

    print_card_review_back(stdout, card.card_as_mut_ref(), true);
    loop {
        match get_char() {
            'q' => return SomeStatus::Break,
            'e' => {
                *card = card.edit_with_vim();
                print_card_review_full(stdout, card.card_as_mut_ref());
            }
            'j' => {
                card.0.meta.suspended = true;
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
    SomeStatus::Continue
}

fn view_cards(stdout: &mut Stdout, mut cards: Vec<AnnoCard>) {
    if cards.is_empty() {
        draw_message(stdout, "No cards found");
        return;
    }

    let mut selected = 0;

    loop {
        let card_qty = cards.len();
        let mut card = &mut cards[selected];

        let message = format!(
            "{}/{}\t{}\n{}\n-------------------\n{}",
            selected + 1,
            card_qty,
            card.1.category.print_full(),
            card.0.front.text,
            card.0.back.text
        );

        match draw_message(stdout, &message) {
            KeyCode::Char('l') | KeyCode::Right if selected != card_qty - 1 => selected += 1,
            KeyCode::Char('h') | KeyCode::Left if selected != 0 => selected -= 1,
            KeyCode::Char('r') => {
                cards.remove(selected);
                if cards.is_empty() {
                    draw_message(stdout, "No more cards");
                    return;
                }
                if selected == cards.len() {
                    selected -= 1;
                }
            }
            KeyCode::Char('f') => {
                card.0.meta.finished = true;
                *card = card.clone().update_card();
            }
            KeyCode::Char('D') => {
                card.clone().delete();
                draw_message(stdout, "Card deleted");
                cards.remove(selected);
                if cards.is_empty() {
                    draw_message(stdout, "No more cards");
                    return;
                }
                if selected == cards.len() {
                    selected -= 1;
                }
            }
            KeyCode::Char('s') => {
                card.0.meta.suspended = true;
                *card = card.clone().update_card();
                draw_message(stdout, "Card suspended");
                cards.remove(selected);
                if cards.is_empty() {
                    draw_message(stdout, "No more cards");
                    return;
                }
                if selected == cards.len() {
                    selected -= 1;
                }
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
            KeyCode::Char('g') => {
                let tags = card.1.category.get_tags().into_iter().collect();
                let tag = match pick_item(stdout, &tags) {
                    Some(tag) => tag,
                    None => continue,
                };
                card.0.meta.tags.insert(tag.to_owned());
                *card = card.update_card();
            }

            KeyCode::Char('T') => {
                draw_message(stdout, "Adding new dependent");
                if let Some(updated_card) =
                    add_dependent(stdout, card.to_owned(), Some(&card.1.category))
                {
                    *card = updated_card;
                }
            }
            KeyCode::Char('Y') => {
                draw_message(stdout, "Adding new dependency");
                if let Some(updated_card) =
                    add_dependency(stdout, card.to_owned(), Some(&card.1.category))
                {
                    *card = updated_card;
                }
            }
            KeyCode::Char('e') => {
                *card = card.edit_with_vim();
            }
            key if should_exit(&key) => return,
            _ => {}
        };
    }
}

fn rev_cards(stdout: &mut Stdout, mut cards: Vec<AnnoCard>) -> bool {
    let qty = cards.len();

    for (i, card) in cards.iter_mut().enumerate() {
        execute!(stdout, Clear(ClearType::All)).unwrap();
        update_card_review_status(stdout, i, qty, &card.1.category);
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

fn get_following_unfinished_cards(category: &Category) -> Vec<AnnoCard> {
    let categories = category.get_following_categories();
    let mut cards = vec![];
    for category in categories {
        cards.extend(category.get_unfinished_cards());
    }
    cards
}

pub fn review_cards(
    stdout: &mut Stdout,
    category: Category,
    mut get_cards: Box<dyn FnMut(&Category) -> Vec<AnnoCard>>,
) {
    let categories = category.get_following_categories();

    for category in categories {
        let mut cards = get_cards(&category);
        let status = "foobar".to_string();
        for card in cards.iter_mut() {
            match {
                match card.get_review_type() {
                    ReviewType::Normal | ReviewType::Pending => {
                        review_card(stdout, card, status.clone())
                    }

                    ReviewType::Unfinished => review_unfinished_card(stdout, card),
                }
            } {
                SomeStatus::Continue => continue,
                SomeStatus::Break => return,
            }
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

/// Fixes the problem where printing a newline doesn't make the cursor go to the left
fn write_string(stdout: &mut Stdout, message: &str) {
    for char in message.chars() {
        print!("{char}");
        if char == '\n' {
            move_far_left(stdout);
        }
    }
}

pub fn draw_message(stdout: &mut Stdout, message: &str) -> KeyCode {
    execute!(stdout, MoveTo(0, 0)).unwrap();

    execute!(stdout, Clear(ClearType::All)).unwrap();
    write_string(stdout, message);
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
    if items.is_empty() {
        draw_message(stdout, "list is empty");
        return None;
    }
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
    category: Option<&Category>,
) -> Option<AnnoCard> {
    let category = category.unwrap_or(&card.1.category);
    let new_dependency = add_card(stdout, category)?;
    card.0.meta.dependencies.push(new_dependency.0.meta.id);
    Some(card.update_card())
}

pub fn add_dependent(
    stdout: &mut Stdout,
    mut card: AnnoCard,
    category: Option<&Category>,
) -> Option<AnnoCard> {
    let category = category.cloned().unwrap_or_else(|| card.1.category.clone());
    let new_dependent = add_card(stdout, &category)?;
    card.0.meta.dependents.push(new_dependent.0.meta.id);
    Some(card.update_card())
}

pub fn pick_card_from_search(stdout: &mut Stdout) -> Option<AnnoCard> {
    let input = read_user_input(stdout)?;
    let cards = AnnoCard::search(input.0);
    pick_item(stdout, &cards).cloned()
}
