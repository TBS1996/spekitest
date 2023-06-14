//! this will be about actually using the program like reviewing and all that

use std::io::{stdout, Stdout};

use crate::card::Card;
use crate::categories::Category;
use crate::common::open_file_with_vim;
use crate::config::Config;
use crate::folders::{
    get_path_from_id, get_pending_cards_from_category, get_review_cards_from_category,
    open_share_path_in_explorer,
};
use crate::git::git_save;

pub fn run(config: Config) {
    enable_raw_mode().unwrap();
    let mut stdout = stdout();
    execute!(stdout, Hide).unwrap();

    let menu_items = vec![
        "Add new cards",
        "Review cards",
        "Review pending cards",
        "Open in file explorer",
    ];

    while let Some(choice) = draw_menu(&mut stdout, &menu_items, true) {
        match choice {
            0 => {
                let category = match choose_folder(&mut stdout, true) {
                    Some(category) => category,
                    None => continue,
                };
                add_cards(&mut stdout, category, true);
                let has_remote = config.read_git_remote().is_some();
                let _ = std::thread::spawn(move || git_save(has_remote));
            }
            1 => {
                let category = match choose_folder(&mut stdout, true) {
                    Some(category) => category,
                    None => continue,
                };
                review_cards(&mut stdout, category);
                let has_remote = config.read_git_remote().is_some();
                let _ = std::thread::spawn(move || git_save(has_remote));
            }
            2 => {
                let category = match choose_folder(&mut stdout, true) {
                    Some(category) => category,
                    None => continue,
                };
                review_pending_cards(&mut stdout, category);
                let has_remote = config.read_git_remote().is_some();
                let _ = std::thread::spawn(move || git_save(has_remote));
            }
            3 => open_share_path_in_explorer().unwrap(),
            _ => {}
        };
    }
    execute!(stdout, Show).unwrap();
    disable_raw_mode().unwrap();
}

use std::io::Write;

pub fn read_user_input(stdout: &mut Stdout) -> Option<String> {
    let mut input = String::new();

    loop {
        if let Event::Key(event) = read().unwrap() {
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
                KeyCode::Esc => return None,
                _ => {}
            }
        }
    }
    Some(input)
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

pub fn add_cards(stdout: &mut Stdout, category: Category, finished: bool) {
    loop {
        execute!(stdout, Clear(ClearType::All)).unwrap();
        execute!(stdout, MoveTo(0, 1)).unwrap();
        update_status_bar(stdout, "--front side--");

        let front_text = match read_user_input(stdout) {
            Some(text) => text,
            None => return,
        };

        execute!(stdout, MoveDown(2)).unwrap();
        move_far_left(stdout);
        println!("--back side--");
        move_far_left(stdout);

        let back_text = match read_user_input(stdout) {
            Some(text) => text,
            None => return,
        };

        let mut card = Card::new_simple(front_text, back_text);

        if !finished {
            card.meta.finished = false;
        }

        card.save_card(Some(category.clone()));
    }
}

pub fn review_pending_cards(stdout: &mut Stdout, category: Category) {
    let categories = category.get_following_categories();

    for category in categories {
        let cards = get_pending_cards_from_category(&category);
        if rev_cards(stdout, cards, &category) {
            return;
        }
    }
    draw_message(stdout, "Nothing left to review!");
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

fn rev_cards(stdout: &mut Stdout, mut cards: Vec<Card>, category: &Category) -> bool {
    let qty = cards.len();

    for (i, card) in cards.iter_mut().enumerate() {
        execute!(stdout, Clear(ClearType::All)).unwrap();
        update_card_review_status(stdout, i, qty, category);
        print_card_review_front(stdout, card, true);

        if should_exit(&get_keycode()) {
            return false;
        }

        print_card_review_back(stdout, card, true);
        loop {
            match get_char() {
                'q' => return false,
                'e' => {
                    open_file_with_vim(get_path_from_id(card.meta.id, category).unwrap()).unwrap();
                    *card = Card::load_from_id(card.meta.id).unwrap();
                    print_card_review_full(stdout, card);
                }
                'j' => {
                    card.meta.suspended = true;
                    card.clone().save_card(Some(category.to_owned()));
                    draw_message(stdout, "card suspended");
                    break;
                }
                // skip card
                's' => break,

                c => match c.to_string().parse() {
                    Ok(grade) => {
                        card.new_review(grade, category);
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
        let cards = get_review_cards_from_category(&category);
        if rev_cards(stdout, cards, &category) {
            return;
        }
    }
    draw_message(stdout, "Nothing left to review!");
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
    println!("{}", message);
    execute!(stdout, ResetColor).unwrap();

    let pressed_char = get_keycode();

    execute!(stdout, Clear(ClearType::All)).unwrap();

    pressed_char
}

fn choose_folder(stdout: &mut Stdout, optional: bool) -> Option<Category> {
    let mut folders = Category::load_all().unwrap();
    Category::sort_categories(&mut folders);

    let mut items: Vec<String> = folders
        .clone()
        .into_iter()
        .map(|cat| cat.print_it_with_depth())
        .collect();

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
                KeyCode::Char('n') => {
                    if let Some(folder_name) = read_user_input(stdout) {
                        let selected_category = folders[selected].clone();
                        let new_category = selected_category._append(&folder_name);
                        new_category.create();
                        folders = Category::load_all().unwrap();
                        Category::sort_categories(&mut folders);
                        items = folders
                            .clone()
                            .into_iter()
                            .map(|cat| cat.print_it_with_depth())
                            .collect();
                    }
                }
                KeyCode::Enter | KeyCode::Char(' ') => {
                    execute!(stdout, Clear(ClearType::All)).unwrap();
                    execute!(stdout, MoveTo(0, items.len() as u16 + 1)).unwrap();
                    return Some(folders[selected].clone());
                }
                KeyCode::Char('q') | KeyCode::Esc if optional => return None,
                _ => {}
            }
        }
    }
}

fn draw_menu(stdout: &mut Stdout, items: &[&str], optional: bool) -> Option<usize> {
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
