//! this will be about actually using the program like reviewing and all that

use std::fmt::Display;
use std::io::{stdout, Stdout};
use std::time::Duration;

use crate::card::{
    calculate_left_memory, AnnoCard, Card, CardLocationCache, IsSuspended, ReviewType,
};
use crate::categories::Category;
use crate::common::{current_time, open_file_with_vim, randvec, truncate_string};
use crate::config::Config;
use crate::folders::view_cards_in_explorer;
use crate::git::git_save;
use crate::paths::get_share_path;

pub fn suspend_card(stdout: &mut Stdout, card: &mut AnnoCard) {
    draw_message(stdout, "hey how many days do you wanna suspend?");

    loop {
        if let Some((input, _)) = read_user_input(stdout) {
            if let Ok(num) = input.parse::<f32>() {
                let days = Duration::from_secs_f32(86400. * num);
                let until = days + current_time();
                card.card.meta.suspended = IsSuspended::TrueUntil(until);
                card.update_card();
                draw_message(stdout, "Card suspended");
                return;
            }
        } else {
            draw_message(stdout, "Card not suspended");
            return;
        }
    }
}

pub fn health_check(_stdout: &mut Stdout) {
    let all_cards = AnnoCard::load_all();

    for card in all_cards {
        let id = card.card.meta.id;

        for dependency in card.card.meta.dependencies {
            let mut dependency = AnnoCard::from_id(&dependency).unwrap();
            dependency.set_dependent(&id);
        }

        for dependent in card.card.meta.dependents {
            let mut dependent = AnnoCard::from_id(&dependent).unwrap();
            dependent.set_dependency(&id);
        }
    }
}

pub fn clear_window(stdout: &mut Stdout) {
    execute!(stdout, Clear(ClearType::All)).unwrap();
}

pub fn move_upper_left(stdout: &mut Stdout) {
    execute!(stdout, MoveTo(0, 0)).unwrap()
}

pub fn view_dependencies(stdout: &mut Stdout, card: &mut AnnoCard, cache: &mut CardLocationCache) {
    let mut msg = String::from("Dependents:\n");

    let dependents = card.get_dependents(cache);
    for dep in dependents {
        msg.push_str(&format!(
            "   {}\tfinished: {}\n",
            truncate_string(dep.card.front.text, 50),
            dep.card.meta.finished,
        ));
    }
    msg.push('\n');
    msg.push('\n');

    let dependencies = card.get_dependencies(cache);
    msg.push_str("Dependencies:\n");
    for dep in dependencies {
        msg.push_str(&format!(
            "   {}\tfinished: {}\n",
            truncate_string(dep.card.front.text, 50),
            dep.card.meta.finished,
        ));
    }

    draw_message(stdout, &msg);
}

pub fn print_cool_graph(stdout: &mut Stdout, data: Vec<f64>, message: &str) {
    let (_, height) = crossterm::terminal::size().unwrap();

    clear_window(stdout);
    move_upper_left(stdout);

    let output = rasciigraph::plot(
        data,
        rasciigraph::Config::default().with_height(height as u32 - 4),
    );

    let output = format!("{}\n_____________\n{}", message, output);

    write_string(stdout, &output);

    read().unwrap();
}

fn string_reverse() -> String {
    let mut old_string = String::from("foo bar baz");
    let mut reversed_string = String::new();

    while let Some(ch) = old_string.pop() {
        reversed_string.push(ch);
    }
    reversed_string
}

pub fn print_cool_graphs(stdout: &mut Stdout, cache: &mut CardLocationCache) {
    let mut all_cards = AnnoCard::load_all();
    all_cards.retain(|card| card.is_resolved(cache));

    let mut vec = vec![0; 50];
    let mut max_stab = 0;

    for card in &all_cards {
        let Some(stability) = card.card.meta.stability else {continue};
        let stability = stability.as_secs() / (86400 / 4);
        if stability > max_stab {
            max_stab = stability;
        }
        if stability < 50 {
            vec[stability as usize] += 1;
        }
    }

    vec.truncate(max_stab as usize + 1);

    let newvec = vec.into_iter().map(|num| num as f64).collect();

    print_cool_graph(stdout, newvec, "Stability distribution");

    let mut rev_vec = vec![0; 50];

    for days in 0..50 {
        print!("{} ", days);
        let mut count = 0;
        for card in &all_cards {
            let Some(mut time_passed)  = card.card.time_since_last_review() else {continue};
            time_passed += std::time::Duration::from_secs(86400 * days / 4);
            let Some(stability) = card.card.meta.stability else {continue};
            if Card::calculate_strength(time_passed, stability) < 0.9 {
                count += 1;
            }
        }

        rev_vec[days as usize] = count;
    }

    let rev_vec: Vec<f64> = rev_vec.into_iter().map(|num| num as f64).collect();
    print_cool_graph(stdout, rev_vec, "Review distribution");

    let mut strengthvec = vec![0; 1000];
    let mut accum = vec![];

    let mut max_strength = 0;
    let mut tot_strength = 0.;
    for card in &all_cards {
        let Some(stability) = card.card.meta.stability else {continue};
        let Some(days_passed) = card.card.time_since_last_review() else {continue};

        let strength = calculate_left_memory(days_passed, stability);
        tot_strength += strength;
        let strength = strength as u32;
        if strength > max_strength {
            max_strength = strength;
        }
        strengthvec[strength as usize] += 1;
        accum.push(strength);
    }

    strengthvec.truncate(max_strength as usize + 50);

    let accum = strengthvec.into_iter().map(|num| num as f64).collect();

    //accum.sort();

    //let accum = accum.into_iter().map(|num| num as f64).collect();

    print_cool_graph(
        stdout,
        accum,
        &format!(
            "Strength distribution\ttot: {} days",
            (tot_strength / 1.) as u32
        ),
    );

    let mut recall_vec = vec![];
    for card in &all_cards {
        if let Some(recall) = card.card.recall_rate() {
            recall_vec.push((recall * 100.) as i32);
        }
    }

    recall_vec.sort_by(|a, b| b.cmp(a));
    recall_vec.retain(|num| *num % 2 == 0);
    let recall_vec = recall_vec.into_iter().map(|n| n as f64).collect();

    print_cool_graph(stdout, recall_vec, "Recall distribution");
}

pub fn card_options(stdout: &mut Stdout, card: &mut AnnoCard, cache: &mut CardLocationCache) {
    let options = vec![
        "Add new dependency",
        "Add new dependent",
        "Add existing dependency",
        "Add existing dependent",
        "View dependency stuff",
    ];
    loop {
        let Some(choice) = draw_menu(stdout, options.clone(), true) else {return};

        match choice {
            0 => {
                draw_message(stdout, "Adding new dependency");
                if let Some(updated_card) = add_dependency(
                    stdout,
                    card.to_owned(),
                    Some(&card.location.category),
                    cache,
                ) {
                    *card = updated_card;
                }
            }
            1 => {
                draw_message(stdout, "Adding new dependent");
                if let Some(updated_card) = add_dependent(
                    stdout,
                    card.to_owned(),
                    Some(&card.location.category),
                    cache,
                ) {
                    *card = updated_card;
                }
            }
            2 => {
                *card = match pick_card_from_search(stdout) {
                    Some(chosen_card) => {
                        card.card.meta.dependencies.insert(chosen_card.card.meta.id);
                        card.update_card()
                    }
                    None => continue,
                }
            }
            3 => {
                *card = match pick_card_from_search(stdout) {
                    Some(chosen_card) => {
                        card.card.meta.dependents.insert(chosen_card.card.meta.id);
                        card.update_card()
                    }
                    None => continue,
                }
            }
            4 => view_dependencies(stdout, card, cache),

            _ => panic!(),
        }
    }
}

pub fn run() {
    let mut cache = CardLocationCache::new();
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
        "notes",
        "pretty graph",
        "health check",
    ];

    while let Some(choice) = draw_menu(&mut stdout, menu_items.clone(), true) {
        match choice {
            0 => {
                let Some(category) =  choose_folder(&mut stdout, "Folder to add card to")  else {continue};
                add_cards(&mut stdout, category, &mut cache);
                let has_remote = Config::load().unwrap().git_remote.is_some();
                let _ = std::thread::spawn(move || git_save(has_remote));
            }
            1 => {
                let Some(revtype) = draw_menu(&mut stdout, vec!["Normal", "Pending", "Unfinished"], true) else {continue};

                let Some(category) =  choose_folder(&mut stdout, "Choose review type") else {continue};

                match revtype {
                    0 => {
                        review_cards(
                            &mut stdout,
                            category.clone(),
                            Box::new(Category::get_review_cards),
                            &mut cache,
                        );
                        draw_message(&mut stdout, "now reviewing pending cards");
                        review_cards(
                            &mut stdout,
                            category.clone(),
                            Box::new(Category::get_pending_cards),
                            &mut cache,
                        );
                    }
                    1 => {
                        review_cards(
                            &mut stdout,
                            category.clone(),
                            Box::new(Category::get_pending_cards),
                            &mut cache,
                        );
                    }
                    2 => {
                        let cards = get_following_unfinished_cards(&category, &mut cache);
                        view_cards(&mut stdout, cards, &mut cache);
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
                let mut cards = AnnoCard::load_all();
                cards.sort_by_key(|card| card.last_modified);
                cards.reverse();
                view_cards(&mut stdout, cards, &mut cache);
            }
            5 => {
                if let Some(card) = search_for_item(&mut stdout, "find some card") {
                    let mut cards = AnnoCard::load_all();
                    cards.insert(0, card);
                    view_cards(&mut stdout, cards, &mut cache);
                }
            }
            6 => {
                let tags: Vec<String> = Category::get_all_tags().into_iter().collect();
                let tag = pick_item(&mut stdout, "Tag to filter by", &tags);
                if let Some(tag) = tag {
                    let cards = AnnoCard::load_all()
                        .into_iter()
                        .filter(|card| card.card.meta.tags.contains(tag))
                        .collect();
                    view_cards(&mut stdout, cards, &mut cache);
                }
            }
            7 => {
                let path = get_share_path().join("notes");
                open_file_with_vim(path.as_path()).unwrap();
            }
            8 => {
                print_cool_graphs(&mut stdout, &mut cache);
            }
            9 => {
                health_check(&mut stdout);
            }
            _ => {}
        };
    }
    execute!(stdout, Clear(ClearType::All)).unwrap();
    execute!(stdout, Show).unwrap();
    disable_raw_mode().unwrap();
}

use std::io::Write;

pub fn search_for_item(stdout: &mut Stdout, message: &str) -> Option<AnnoCard> {
    let mut input = String::new();

    let cards = AnnoCard::load_all();
    let mut index = 0;

    let mut print_stuff = |search_term: &str, cards: Vec<&AnnoCard>, index: &mut usize| {
        clear_window(stdout);
        //move_upper_left(stdout);
        execute!(stdout, MoveTo(0, 0)).unwrap();
        println!("{}", message);
        println!("\t\t| {} |", search_term);
        let screen_height = crossterm::terminal::size().unwrap().1 - 10;
        *index = std::cmp::min(
            std::cmp::min(*index, screen_height.into()),
            cards.len().saturating_sub(1),
        );
        for (idx, card) in cards.iter().enumerate() {
            move_far_left(stdout);

            if idx == *index {
                execute!(stdout, SetForegroundColor(crossterm::style::Color::Blue)).unwrap();
                println!("> {}", card.card.front.text);
                execute!(stdout, ResetColor).unwrap();
            } else {
                println!("  {}", card.card.front.text);
            }

            if idx == screen_height.into() {
                break;
            }
        }
    };

    loop {
        if let Event::Key(event) = read().unwrap() {
            match event.code {
                KeyCode::Char(c) => {
                    input.push(c);
                    let the_cards = AnnoCard::search_in_cards(&input, &cards);
                    print_stuff(&input, the_cards, &mut index);
                }
                KeyCode::Backspace if !input.is_empty() => {
                    input.pop();
                    let the_cards = AnnoCard::search_in_cards(&input, &cards);
                    print_stuff(&input, the_cards, &mut index);
                }
                KeyCode::Enter => {
                    let the_cards = AnnoCard::search_in_cards(&input, &cards);
                    return Some(the_cards[index].to_owned());
                }
                KeyCode::Down => {
                    index += 1;

                    let the_cards = AnnoCard::search_in_cards(&input, &cards);
                    print_stuff(&input, the_cards, &mut index);
                }
                KeyCode::Up => {
                    let the_cards = AnnoCard::search_in_cards(&input, &cards);
                    index = index.saturating_sub(1);
                    print_stuff(&input, the_cards, &mut index);
                }
                KeyCode::Esc => return None,
                KeyCode::Left => println!("{index}"),
                _ => {}
            }
        }
    }
}

pub fn read_user_input(stdout: &mut Stdout) -> Option<(String, KeyCode)> {
    let mut input = String::new();
    let mut key_code;

    loop {
        if let Event::Key(event) = read().unwrap() {
            key_code = event.code;
            match event.code {
                KeyCode::Char('`') => break,
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

pub fn add_card(
    stdout: &mut Stdout,
    category: &mut Category,
    cache: &mut CardLocationCache,
) -> Option<AnnoCard> {
    loop {
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

        if code == KeyCode::Char('`') {
            if let Some(the_category) = choose_folder(stdout, "Choose new category") {
                *category = the_category;
            }
            continue;
        }

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

        return Some(card.save_new_card(category, cache));
    }
}

pub fn add_cards(stdout: &mut Stdout, mut category: Category, cache: &mut CardLocationCache) {
    loop {
        if add_card(stdout, &mut category, cache).is_none() {
            return;
        }
    }
}

pub enum SomeStatus {
    Continue,
    Break,
}

fn review_unfinished_card(
    stdout: &mut Stdout,
    card: &mut AnnoCard,
    cache: &mut CardLocationCache,
) -> SomeStatus {
    let get_message = |card: &AnnoCard| {
        format!(
            "{}\n-------------------\n{}",
            card.card.front.text, card.card.back.text
        )
    };

    loop {
        match draw_message(stdout, &get_message(card)) {
            KeyCode::Char('f') => {
                card.card.meta.finished = true;
                card.update_card();
            }
            KeyCode::Char('s') => break,
            KeyCode::Char('y') => {
                *card = match pick_card_from_search(stdout) {
                    Some(chosen_card) => {
                        card.card.meta.dependencies.insert(chosen_card.card.meta.id);
                        card.update_card()
                    }
                    None => continue,
                }
            }
            KeyCode::Char('t') => {
                *card = match pick_card_from_search(stdout) {
                    Some(chosen_card) => {
                        card.card.meta.dependents.insert(chosen_card.card.meta.id);
                        card.update_card()
                    }
                    None => continue,
                }
            }
            KeyCode::Char('g') => {
                let tags = card.location.category.get_tags().into_iter().collect();
                let tag = match pick_item(stdout, "Choose tag", &tags) {
                    Some(tag) => tag,
                    None => continue,
                };
                card.card.meta.tags.insert(tag.to_owned());
                *card = card.update_card();
            }

            KeyCode::Char('T') => {
                draw_message(stdout, "Adding new dependent");
                if let Some(updated_card) = add_dependent(
                    stdout,
                    card.to_owned(),
                    Some(&card.location.category),
                    cache,
                ) {
                    *card = updated_card;
                }
            }
            KeyCode::Char('Y') => {
                draw_message(stdout, "Adding new dependency");
                if let Some(updated_card) = add_dependency(
                    stdout,
                    card.to_owned(),
                    Some(&card.location.category),
                    cache,
                ) {
                    *card = updated_card;
                }
            }
            KeyCode::Char('D') => {
                card.clone().delete(cache);
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

pub fn review_unfinished_cards(
    stdout: &mut Stdout,
    category: Category,
    cache: &mut CardLocationCache,
) {
    let mut cards = category.get_unfinished_cards(cache);
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
                &card.location.category.print_full(),
                card.card.front.text,
                card.card.back.text
            )
        };

        match draw_message(stdout, &get_message(card)) {
            KeyCode::Char('f') => {
                card.card.meta.finished = true;
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
                        card.card.meta.dependencies.insert(chosen_card.card.meta.id);
                        card.update_card()
                    }
                    None => continue,
                }
            }
            KeyCode::Char('t') => {
                *card = match pick_card_from_search(stdout) {
                    Some(chosen_card) => {
                        card.card.meta.dependents.insert(chosen_card.card.meta.id);
                        card.update_card()
                    }
                    None => continue,
                }
            }
            KeyCode::Char('g') => {
                let tags = card.location.category.get_tags().into_iter().collect();
                let tag = match pick_item(stdout, "Choose tag", &tags) {
                    Some(tag) => tag,
                    None => continue,
                };
                card.card.meta.tags.insert(tag.to_owned());
                *card = card.update_card();
            }

            KeyCode::Char('T') => {
                draw_message(stdout, "Adding new dependent");
                if let Some(updated_card) =
                    add_dependent(stdout, card.to_owned(), Some(&category.clone()), cache)
                {
                    *card = updated_card;
                }
            }
            KeyCode::Char('Y') => {
                draw_message(stdout, "Adding new dependency");
                if let Some(updated_card) =
                    add_dependency(stdout, card.to_owned(), Some(&category.clone()), cache)
                {
                    *card = updated_card;
                }
            }
            KeyCode::Char('D') => {
                let the_card = cards.remove(selected);
                the_card.delete(cache);
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

pub fn review_pending_cards(
    stdout: &mut Stdout,
    category: Category,
    cache: &mut CardLocationCache,
) {
    let categories = category.get_following_categories();

    for category in categories {
        let cards = category.get_pending_cards(cache);
        if rev_cards(stdout, cards, cache) {
            return;
        }
    }
}

fn update_card_review_status(i: usize, qty: usize, category: &Category) -> String {
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

fn review_card(
    stdout: &mut Stdout,
    card: &mut AnnoCard,
    status: String,
    cache: &mut CardLocationCache,
) -> SomeStatus {
    let mut print_front = || {
        execute!(stdout, Clear(ClearType::All)).unwrap();
        update_status_bar(stdout, &status);
        print_card_review_front(stdout, card.card_as_mut_ref(), true);
    };

    let print_all = |stdout: &mut Stdout, card: &mut AnnoCard| {
        execute!(stdout, Clear(ClearType::All)).unwrap();
        update_status_bar(stdout, &status);
        print_card_review_front(stdout, card.card_as_mut_ref(), true);
        print_card_review_back(stdout, card.card_as_mut_ref(), true);
    };

    print_front();

    let key = get_keycode();

    if should_exit(&key) {
        return SomeStatus::Break;
    }

    if key == KeyCode::Char('`') {
        card_options(stdout, card, cache);
        execute!(stdout, Clear(ClearType::All)).unwrap();
        update_status_bar(stdout, &status);
        print_card_review_front(stdout, card.card_as_mut_ref(), true);
    }

    print_card_review_back(stdout, card.card_as_mut_ref(), true);
    loop {
        print_all(stdout, card);
        match get_char() {
            '`' => {
                card_options(stdout, card, cache);
                execute!(stdout, Clear(ClearType::All)).unwrap();
                update_status_bar(stdout, &status);
                print_card_review_front(stdout, card.card_as_mut_ref(), true);
                print_card_review_back(stdout, card.card_as_mut_ref(), true);
            }

            'Y' => {
                draw_message(stdout, "Adding new dependency");
                if let Some(updated_card) = add_dependency(
                    stdout,
                    card.to_owned(),
                    Some(&card.location.category),
                    cache,
                ) {
                    *card = updated_card;
                }
            }
            'T' => {
                draw_message(stdout, "Adding new dependent");
                if let Some(updated_card) = add_dependent(
                    stdout,
                    card.to_owned(),
                    Some(&card.location.category),
                    cache,
                ) {
                    *card = updated_card;
                }
            }
            'y' => {
                if let Some(chosen_card) = search_for_item(stdout, "Add dependency") {
                    card.set_dependency(&chosen_card.card.meta.id);
                }
                continue;
            }
            't' => {
                if let Some(chosen_card) = search_for_item(stdout, "Add dependent") {
                    card.set_dependent(&chosen_card.card.meta.id);
                }
                continue;
            }
            'v' => {
                view_dependencies(stdout, card, cache);
            }
            'q' => return SomeStatus::Break,
            'e' => {
                *card = card.edit_with_vim();
                print_card_review_full(stdout, card.card_as_mut_ref());
            }
            // skip card
            's' => break,
            'S' => suspend_card(stdout, card),
            'a' => {
                add_card(stdout, &mut card.clone().location.category, cache);
            }

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

fn view_cards(stdout: &mut Stdout, mut cards: Vec<AnnoCard>, cache: &mut CardLocationCache) {
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
            card.location.category.print_full(),
            card.card.front.text,
            card.card.back.text
        );

        let key_event = draw_key_event_message(stdout, &message);
        match key_event.code {
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
            KeyCode::Char('`') => {
                card_options(stdout, card, cache);
            }
            KeyCode::Char('a') => {
                add_card(stdout, &mut card.location.category.clone(), cache);
            }
            KeyCode::Char('f') => {
                card.card.meta.finished = true;
                *card = card.clone().update_card();
            }
            KeyCode::Char('D') => {
                card.clone().delete(cache);
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
            KeyCode::Char('s') => {}
            KeyCode::Char('S') => suspend_card(stdout, card),

            KeyCode::Char('g') => {
                let tags = card.location.category.get_tags().into_iter().collect();
                let tag = match pick_item(stdout, "Choose tag", &tags) {
                    Some(tag) => tag,
                    None => continue,
                };
                card.card.meta.tags.insert(tag.to_owned());
                *card = card.update_card();
            }

            KeyCode::Char('T') => {
                draw_message(stdout, "Adding new dependent");
                if let Some(updated_card) = add_dependent(
                    stdout,
                    card.to_owned(),
                    Some(&card.location.category),
                    cache,
                ) {
                    *card = updated_card;
                }
            }
            KeyCode::Char('Y') => {
                draw_message(stdout, "Adding new dependency");
                if let Some(updated_card) = add_dependency(
                    stdout,
                    card.to_owned(),
                    Some(&card.location.category),
                    cache,
                ) {
                    *card = updated_card;
                }
            }
            KeyCode::Char('y') => {
                if let Some(chosen_card) = search_for_item(stdout, "Add dependency") {
                    card.set_dependency(&chosen_card.card.meta.id);
                }
                continue;
            }
            KeyCode::Char('t') => {
                if let Some(chosen_card) = search_for_item(stdout, "Add dependent") {
                    card.set_dependent(&chosen_card.card.meta.id);
                }
                continue;
            }
            KeyCode::Char('v') => {
                view_dependencies(stdout, card, cache);
            }
            KeyCode::Char('m') => {
                let folder = match choose_folder(stdout, "Move card to...") {
                    Some(folder) => folder,
                    None => continue,
                };

                *card = card.clone().move_card(&folder, cache);
            }
            KeyCode::Char('e') => {
                *card = card.edit_with_vim();
            }
            key if should_exit(&key) => return,
            _ => {}
        };
    }
}

fn rev_cards(stdout: &mut Stdout, mut cards: Vec<AnnoCard>, cache: &mut CardLocationCache) -> bool {
    let qty = cards.len();

    for (i, card) in cards.iter_mut().enumerate() {
        execute!(stdout, Clear(ClearType::All)).unwrap();
        update_card_review_status(i, qty, &card.location.category);
        print_card_review_front(stdout, card.card_as_mut_ref(), true);

        let key = get_keycode();

        if should_exit(&key) {
            return false;
        }

        if key == KeyCode::Char('`') {
            card_options(stdout, card, cache);
        }

        print_card_review_back(stdout, card.card_as_mut_ref(), true);
        loop {
            match get_char() {
                'q' => return false,
                'e' => {
                    *card = card.edit_with_vim();
                    print_card_review_full(stdout, card.card_as_mut_ref());
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

fn get_following_unfinished_cards(
    category: &Category,
    cache: &mut CardLocationCache,
) -> Vec<AnnoCard> {
    let categories = category.get_following_categories();
    let mut cards = vec![];
    for category in categories {
        cards.extend(category.get_unfinished_cards(cache));
    }
    randvec(cards)
}

pub type CardsFromCategory = Box<dyn FnMut(&Category, &mut CardLocationCache) -> Vec<AnnoCard>>;

pub fn review_cards(
    stdout: &mut Stdout,
    category: Category,
    mut get_cards: CardsFromCategory,
    cache: &mut CardLocationCache,
) {
    let categories = category.get_following_categories();

    for category in &categories {
        let mut cards = get_cards(category, cache);

        let cardqty = cards.len();
        for (index, card) in cards.iter_mut().enumerate() {
            let status = format!(
                "{}/{}\t{}\t{}",
                index,
                cardqty,
                category.print_full(),
                card.card.meta.dependencies.len()
            );
            match {
                match card.get_review_type() {
                    ReviewType::Normal | ReviewType::Pending => {
                        review_card(stdout, card, status.clone(), cache)
                    }

                    ReviewType::Unfinished => review_unfinished_card(stdout, card, cache),
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

pub fn get_key_event() -> KeyEvent {
    loop {
        match read().unwrap() {
            Event::Key(event) => return event,
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

pub fn draw_key_event_message(stdout: &mut Stdout, message: &str) -> KeyEvent {
    execute!(stdout, MoveTo(0, 0)).unwrap();

    execute!(stdout, Clear(ClearType::All)).unwrap();
    write_string(stdout, message);
    execute!(stdout, ResetColor).unwrap();

    let pressed_char = get_key_event();

    execute!(stdout, Clear(ClearType::All)).unwrap();

    pressed_char
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

fn choose_folder(stdout: &mut Stdout, message: &str) -> Option<Category> {
    pick_item_with_formatter(
        stdout,
        message,
        &Category::load_all().unwrap(),
        Category::print_it_with_depth,
    )
    .cloned()
}

fn pick_item<'a, T: Display>(
    stdout: &mut Stdout,
    message: &str,
    items: &'a Vec<T>,
) -> Option<&'a T> {
    let formatter = |item: &T| format!("{}", item);
    pick_item_with_formatter(stdout, message, items, formatter)
}

fn pick_item_with_formatter<'a, T, F>(
    stdout: &mut Stdout,
    message: &str,
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
        execute!(stdout, MoveTo(0, 0)).unwrap();
        print!("{}", message);

        for (index, item) in items.iter().enumerate() {
            execute!(stdout, MoveTo(0, (index + 1) as u16)).unwrap();

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
    pick_item(stdout, "", &cards).cloned()
}

pub fn view_search_cards(stdout: &mut Stdout, cache: &mut CardLocationCache) {
    loop {
        let Some((searchterm, _)) = read_user_input(stdout)else {return};
        let mut cards = AnnoCard::search(searchterm);
        cards.truncate(10);
        let Some(picked_card) = pick_item(stdout, "", &cards).cloned() else {return};

        let  Some((idx, _)) = cards
        .iter()
        .enumerate()
        .find(|card| card.1.card.meta.id == picked_card.card.meta.id) else {return};

        cards.remove(idx);
        cards.insert(0, picked_card);
        view_cards(stdout, cards.clone(), cache);
    }
}

pub fn add_dependency(
    stdout: &mut Stdout,
    mut card: AnnoCard,
    category: Option<&Category>,
    cache: &mut CardLocationCache,
) -> Option<AnnoCard> {
    let category = category.unwrap_or(&card.location.category);
    let category = &mut category.to_owned();
    let new_dependency = add_card(stdout, category, cache)?;
    card.card
        .meta
        .dependencies
        .insert(new_dependency.card.meta.id);
    Some(card.update_card())
}

pub fn add_dependent(
    stdout: &mut Stdout,
    mut card: AnnoCard,
    category: Option<&Category>,
    cache: &mut CardLocationCache,
) -> Option<AnnoCard> {
    let mut category = category
        .cloned()
        .unwrap_or_else(|| card.location.category.clone());
    let new_dependent = add_card(stdout, &mut category, cache)?;
    card.card.meta.dependents.insert(new_dependent.card.meta.id);
    Some(card.update_card())
}

pub fn pick_card_from_search(stdout: &mut Stdout) -> Option<AnnoCard> {
    clear_window(stdout);
    move_upper_left(stdout);
    let input = read_user_input(stdout)?;
    let cards = AnnoCard::search(input.0);
    pick_item(stdout, "", &cards).cloned()
}

pub fn cooler_pick_card_from_search(stdout: &mut Stdout) -> Option<AnnoCard> {
    let input = read_user_input(stdout)?;
    let cards = AnnoCard::search(input.0);
    pick_item(stdout, "", &cards).cloned()
}
