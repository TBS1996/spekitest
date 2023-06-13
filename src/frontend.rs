//! this will be about actually using the program like reviewing and all that

use std::io::{stdout, Stdout};

use crate::card::Card;
use crate::common::{open_file_with_vim, Category};
use crate::config::Config;
use crate::folders::{
    get_path_from_id, get_pending_cards_from_category, get_review_cards_from_category,
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
        "Save progress",
        "Quit",
    ];
    loop {
        match draw_menu(&mut stdout, &menu_items, false).unwrap() {
            0 => {
                let category = match pick_category(&mut stdout, true) {
                    Some(category) => category,
                    None => continue,
                };
                add_cards(&mut stdout, category, true);
            }
            1 => {
                let category = match pick_category(&mut stdout, true) {
                    Some(category) => category,
                    None => continue,
                };
                review_cards(&mut stdout, category);
            }
            2 => {
                let category = match pick_category(&mut stdout, true) {
                    Some(category) => category,
                    None => continue,
                };
                review_pending_cards(&mut stdout, category);
            }
            3 => {
                println!("saving progress!");
                git_save(config.read_git_remote().is_some());
            }
            4 => {
                git_save(config.read_git_remote().is_some());
                disable_raw_mode().unwrap();
                return;
            }
            _ => {}
        };
    }
}

fn pick_category(stdout: &mut Stdout, optional: bool) -> Option<Category> {
    let mut categories = Category::load_all().unwrap();
    Category::sort_categories(&mut categories);

    let items_strings: Vec<String> = categories.iter().map(|s| s.print_it_with_depth()).collect();
    let items: Vec<&str> = items_strings.iter().map(|s| s.as_str()).collect();

    draw_menu(stdout, &items, optional).map(|i| categories[i].clone())
}

use std::io::Write;

pub fn read_user_input(stdout: &mut Stdout) -> String {
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
                _ => {}
            }
        }
    }
    input
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
        let front_text = read_user_input(stdout);

        if front_text == "q" {
            return;
        }

        execute!(stdout, MoveDown(2)).unwrap();
        move_far_left(stdout);
        println!("--back side--");
        move_far_left(stdout);
        let back_text = read_user_input(stdout);

        if back_text == "q" {
            return;
        }

        let mut card = Card::new_simple(front_text, back_text);

        if !finished {
            card.meta.finished = false;
        }

        card.save_card(Some(category.clone()));
    }
}

pub fn review_pending_cards(stdout: &mut Stdout, category: Category) {
    let categories = if category.is_root() {
        Category::load_all().unwrap()
    } else {
        vec![category]
    };

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

fn rev_cards(stdout: &mut Stdout, mut cards: Vec<Card>, category: &Category) -> bool {
    execute!(stdout, Clear(ClearType::All)).unwrap();
    let qty = cards.len();

    for (i, card) in cards.iter_mut().enumerate() {
        update_card_review_status(stdout, i, qty, category);
        execute!(stdout, MoveTo(0, 1)).unwrap();
        println!("{}", card.front.text);
        card.front.audio.play_audio();
        get_keycode();
        move_far_left(stdout);
        execute!(stdout, MoveDown(1)).unwrap();
        move_far_left(stdout);
        println!("------------------");
        execute!(stdout, MoveDown(1)).unwrap();
        move_far_left(stdout);

        card.back.audio.play_audio();
        loop {
            match get_char() {
                'q' => return false,
                'e' => {
                    open_file_with_vim(get_path_from_id(card.meta.id, category).unwrap()).unwrap();
                }
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
    let categories = if category.is_root() {
        Category::load_all().unwrap()
    } else {
        vec![category]
    };

    for category in categories {
        let cards = get_review_cards_from_category(&category);
        if rev_cards(stdout, cards, &category) {
            return;
        }
    }
    draw_message(stdout, "Nothing left to review!");
}

use crossterm::cursor::{self, MoveDown, MoveLeft};
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
                    execute!(stdout, MoveTo(0, items.len() as u16 + 1)).unwrap();
                    return Some(selected);
                }
                KeyCode::Char('q') if optional => return None,
                _ => {}
            }
        }
    }
}
