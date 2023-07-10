//! this will be about actually using the program like reviewing and all that

use std::collections::{BTreeSet, HashSet};
use std::fmt::Display;
use std::io::{stdout, Stdout};
use std::sync::Arc;
use std::time::Duration;

use crate::card::{Card, CardCache, IsSuspended, Priority, ReviewType, Reviews, SavedCard};
use crate::categories::Category;
use crate::common::view_cards_in_explorer;
use crate::common::{current_time, open_file_with_vim, randvec, truncate_string};
use crate::config::Config;
use crate::git::git_save;
use crate::paths::get_share_path;
use crate::Id;

use ascii_tree::write_tree;
use ascii_tree::Tree::Node;

fn view_all_cards(stdout: &mut Stdout, cache: &mut CardCache) {
    let cards = cache.all_ids();
    view_cards(stdout, cards, cache);
}

fn to_ascii_tree(
    id: &Id,
    cache: &mut CardCache,
    show_dependencies: bool,
    visited: &mut BTreeSet<Id>,
) -> ascii_tree::Tree {
    visited.insert(*id);

    let card = cache.get_ref(id);
    let mut children = Vec::new();
    let dependencies = if show_dependencies {
        cache.recursive_dependencies(card.id())
    } else {
        cache.recursive_dependents(card.id())
    };

    for dependency in dependencies {
        if !visited.contains(&dependency) {
            children.push(to_ascii_tree(
                &dependency,
                cache,
                show_dependencies,
                visited,
            ));
        }
    }

    visited.remove(id);

    Node(card.front_text().to_owned(), children)
}

fn ascii_test(
    stdout: &mut Stdout,
    card_id: &Id,
    cache: &mut CardCache,
    show_dependencies: bool,
) -> Option<SavedCard> {
    let tree = to_ascii_tree(card_id, cache, show_dependencies, &mut BTreeSet::new());
    let mut output = String::new();

    let msg = if show_dependencies {
        "dependencies"
    } else {
        "dependents"
    };

    let _ = write_tree(&mut output, &tree);

    let lines: Vec<&str> = output.lines().collect();

    if lines.len() == 1 {
        draw_message(stdout, &format!("No {} found", msg));
        return None;
    }

    let item = pick_item(stdout, msg, &lines);

    if let Some(item) = item {
        let cards = if show_dependencies {
            cache.recursive_dependencies(card_id)
        } else {
            cache.recursive_dependents(card_id)
        };

        for card_id in cards {
            let card = cache.get_ref(&card_id);
            if item.contains(card.front_text()) {
                return Some(cache.get_owned(&card_id));
            }
        }
    }
    None
}

fn print_expected_stuff(stdout: &mut Stdout) {
    let mut cards: Vec<SavedCard> = SavedCard::load_all_cards()
        .into_iter()
        .filter(|card| card.stability().is_some())
        .collect();

    cards.sort_by_key(|card| (card.expected_gain().unwrap() * 1000.) as i32);

    let mut s = String::new();

    for card in cards {
        let (Some(gain), Some(stability), Some(recall)) = (card.expected_gain(), card.stability(), card.recall_rate()) else {continue};
        let gain = (gain * 100.).round() / 100.;
        let stability = (stability.as_secs_f32() / 864.).round() / 100.;
        let recall = (recall * 100.).round();
        let whatever = stability < card.time_since_last_review().unwrap().as_secs_f32() / 86400.;

        s.push_str(&format!(
            "gain: {}, stability: {}days, recall: {}%, hey: {}, card: {}\n",
            gain,
            stability,
            recall,
            whatever,
            card.front_text()
        ));
    }
    draw_message(stdout, s.as_str());
}

pub fn affirmative(stdout: &mut Stdout, question: &str) -> bool {
    match draw_menu(stdout, Some(question), vec!["no", "yes"], false).unwrap() {
        0 => false,
        1 => true,
        _ => unreachable!(),
    }
}

fn cards_as_string(cards: &Vec<SavedCard>) -> String {
    let mut s = String::new();

    for card in cards {
        s.push_str(card.front_text());
        s.push('\n');
    }
    s
}

fn print_stats(stdout: &mut Stdout, cache: &mut CardCache) {
    let cards = SavedCard::load_all_cards();
    let all_cards = cards.len();
    let mut suspended = 0;
    let mut finished = 0;
    let mut pending = 0;
    let mut strength = 0;
    let mut reviews = 0;
    let mut resolved = 0;

    for card in cards {
        pending += card.stability().is_none() as i32;
        reviews += card.reviews().len();
        finished += card.is_finished() as i32;
        resolved += card.is_resolved(cache) as i32;
        strength += (card.strength().unwrap_or_default().as_secs_f32() / 86400.).round() as i32;
        suspended += card.is_suspended() as i32;
    }

    let output = format!("suspended: {suspended}\nfinished: {finished}\npending: {pending}\nreviews: {reviews}\nstrength: {strength}\nresolved: {resolved}\ntotal cards: {all_cards}");
    draw_message(stdout, output.as_str());
    print_expected_stuff(stdout);

    let not_confident_cards: Vec<SavedCard> = SavedCard::load_all_cards()
        .into_iter()
        .filter(|card| {
            card.is_resolved(cache)
                && !card.is_confidently_resolved(cache)
                && card.is_finished()
                && !card.is_suspended()
        })
        .collect();
    let s = cards_as_string(&not_confident_cards);
    let s = format!("qty: {}\n{}", not_confident_cards.len(), s);
    draw_message(stdout, &s);
}

pub fn suspend_card(stdout: &mut Stdout, card: &Id, cache: &mut CardCache) {
    let mut card = cache.get_owned(card);
    draw_message(stdout, "hey how many days do you wanna suspend?");

    loop {
        if let Some((input, _)) = read_user_input(stdout) {
            if let Ok(num) = input.parse::<f32>() {
                let days = Duration::from_secs_f32(86400. * num);
                let until = days + current_time();
                card.set_suspended(IsSuspended::TrueUntil(until));
                draw_message(stdout, "Card suspended");
                return;
            }
        } else {
            draw_message(stdout, "Card not suspended");
            return;
        }
    }
}

pub fn health_check(stdout: &mut Stdout, cache: &mut CardCache) {
    cache.refresh();
    let all_cards = SavedCard::load_all_cards();
    move_upper_left(stdout);

    for mut card in all_cards {
        let _id = card.id().to_owned();
        let dependencies = card.dependency_ids().to_owned();
        let dependents = card.dependent_ids().to_owned();

        for d in dependencies {
            if !cache.exists(&d) {
                println!("dependency removed!");
                card.remove_dependency(&d, cache);
            }
        }

        for d in dependents {
            if !cache.exists(&d) {
                println!("dependent removed!");
                card.remove_dependent(&d, cache);
            }
        }

        for dependency in card.dependency_ids() {
            if !cache.exists(dependency) {
                //      card._remove_dependency(&id, cache);
            }

            //let mut dependency = cache._get_owned(dependency);
            //dependency.set_dependent(&id, cache);
        }

        for _dependent in card.dependent_ids() {
            //let mut dependent = cache._get_owned(dependent);
            //dependent.set_dependency(&id, cache);
        }
    }
    cache.refresh();
}

pub fn clear_window(stdout: &mut Stdout) {
    execute!(stdout, Clear(ClearType::All)).unwrap();
}

pub fn move_upper_left(stdout: &mut Stdout) {
    execute!(stdout, MoveTo(0, 0)).unwrap()
}

pub fn view_dependencies(stdout: &mut Stdout, card: &Id, cache: &mut CardCache) {
    let card = cache.get_owned(card);
    let mut msg = String::from("Dependents:\n");

    let dependents = cache.recursive_dependents(card.id());
    for dep in dependents {
        let dep = cache.get_ref(&dep);
        msg.push_str(&format!(
            "   {}\tfinished: {}\n",
            truncate_string(dep.front_text().to_owned(), 50),
            dep.is_finished(),
        ));
    }
    msg.push('\n');
    msg.push('\n');

    let dependencies = cache.recursive_dependencies(card.id());
    msg.push_str("Dependencies:\n");
    for dep in dependencies {
        let dep = cache.get_ref(&dep);
        msg.push_str(&format!(
            "   {}\tfinished: {}\n",
            truncate_string(dep.front_text().to_owned(), 50),
            dep.is_finished(),
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

pub fn print_cool_graphs(stdout: &mut Stdout, cache: &mut CardCache) {
    let mut all_cards = SavedCard::load_all_cards();
    all_cards.retain(|card| card.is_resolved(cache));

    let max = 300;
    let mut vec = vec![0; max];
    let mut max_stab = 0;

    for card in &all_cards {
        let Some(stability) = card.stability() else {continue};
        let stability = stability.as_secs() / (86400 / 4);
        if stability > max_stab {
            max_stab = stability;
        }
        if stability < max as u64 {
            vec[stability as usize] += 1;
        }
    }

    vec.truncate(max_stab as usize + 1);

    let newvec = vec.into_iter().map(|num| num as f64).collect();

    println!("{:?}", &newvec);
    print_cool_graph(stdout, newvec, "Stability distribution");

    let width = crossterm::terminal::size().unwrap().0 - 10;
    let mut rev_vec = vec![0; width as usize];

    for days in 0..width as u32 {
        print!("{} ", days);
        let mut count = 0;
        for card in &all_cards {
            let Some(mut time_passed)  = card.time_since_last_review() else {continue};
            time_passed += std::time::Duration::from_secs((86400 * days / 4).into());
            let Some(stability) = card.stability() else {continue};
            if Reviews::calculate_recall_rate(&time_passed, &stability) < 0.9 {
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
        let Some(strength) = card.strength() else {continue};
        let strength = strength.as_secs_f32() / 86400.;
        /*
        println!(
            "stability: {}, passed: {}, strength {}, card: {} ",
            as_days(stability),
            as_days(&days_passed),
            strength,
            card.front_text()
        );
        move_far_left(stdout);
        */
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
        if let Some(recall) = card.recall_rate() {
            recall_vec.push((recall * 100.) as i32);
        }
    }

    recall_vec.sort_by(|a, b| b.cmp(a));
    recall_vec.retain(|num| *num % 2 == 0);
    let recall_vec = recall_vec.into_iter().map(|n| n as f64).collect();

    print_cool_graph(stdout, recall_vec, "Recall distribution");
}

fn import_stuff(cache: &mut CardCache) {
    let import_path = get_share_path().join("forimport.txt");
    if !import_path.exists() {
        return;
    }
    let category = Category::import_category();
    let cards = Card::import_cards(import_path.as_path());

    if let Some(cards) = cards {
        for card in cards {
            card.save_new_card(&category, cache);
        }
    }
    let to_path = get_share_path().join("imported.txt");
    std::fs::rename(import_path, to_path).unwrap();
}

pub fn run() {
    let mut cache = CardCache::new();
    import_stuff(&mut cache);

    enable_raw_mode().unwrap();
    let mut stdout = stdout();
    execute!(stdout, Hide).unwrap();

    let menu_items = vec![
        "Add new cards",
        "Review cards",
        "View cards",
        "Settings",
        "Debug",
        "by tag",
        "notes",
        "pretty graph",
        "lonely cards",
        "health check",
        "stats",
        "filters",
    ];

    while let Some(choice) = draw_menu(&mut stdout, None, menu_items.clone(), true) {
        match choice {
            0 => {
                let Some(category) =  choose_folder(&mut stdout, "Folder to add card to")  else {continue};
                add_cards(&mut stdout, category, &mut cache);
                let has_remote = Config::load().unwrap().git_remote.is_some();
                let _ = std::thread::spawn(move || git_save(has_remote));
            }
            1 => {
                let Some(revtype) = draw_menu(&mut stdout, None, vec!["Normal", "Pending", "Unfinished"], true) else {continue};

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
                        let mut cards = get_following_unfinished_cards(&category, &mut cache);
                        cards.sort_by_key(|card| {
                            cache.get_ref(card).get_unfinished_dependent_qty(&mut cache)
                        });
                        cards.reverse();
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
                view_all_cards(&mut stdout, &mut cache);
            }
            5 => {
                let tags: Vec<String> = Category::get_all_tags().into_iter().collect();
                let tag = pick_item(&mut stdout, "Tag to filter by", &tags);
                if let Some(tag) = tag {
                    let cards = SavedCard::load_all_cards()
                        .into_iter()
                        .filter_map(|card| card.contains_tag(tag).then(|| card.id().to_owned()))
                        .collect();
                    view_cards(&mut stdout, cards, &mut cache);
                }
            }
            6 => {
                open_file_with_vim(get_share_path().join("notes").as_path()).unwrap();
            }
            7 => {
                print_cool_graphs(&mut stdout, &mut cache);
            }
            8 => {
                let mut cards = SavedCard::load_all_cards()
                    .into_iter()
                    .collect::<Vec<SavedCard>>();
                cards.retain(|card| {
                    card.dependency_ids().is_empty()
                        && card.dependent_ids().is_empty()
                        && card.is_finished()
                });
                let cards = randvec(cards);
                let cards = cards.into_iter().map(|card| card.id().to_owned()).collect();
                view_cards(&mut stdout, cards, &mut cache);
            }
            9 => {
                health_check(&mut stdout, &mut cache);
            }
            10 => print_stats(&mut stdout, &mut cache),
            _ => {}
        };
    }
    execute!(stdout, Clear(ClearType::All)).unwrap();
    execute!(stdout, Show).unwrap();
    disable_raw_mode().unwrap();
}

use std::io::Write;

pub fn search_for_item(
    stdout: &mut Stdout,
    message: &str,
    excluded_cards: HashSet<Id>,
) -> Option<SavedCard> {
    let mut input = String::new();

    let cards = SavedCard::load_all_cards();
    let mut index = 0;

    let mut print_stuff = |search_term: &str, cards: Vec<&SavedCard>, index: &mut usize| {
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
                println!("> {}", card.front_text());
                execute!(stdout, ResetColor).unwrap();
            } else {
                println!("  {}", card.front_text());
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
                    let the_cards = SavedCard::search_in_cards(&input, &cards, &excluded_cards);
                    print_stuff(&input, the_cards, &mut index);
                }
                KeyCode::Backspace if !input.is_empty() => {
                    input.pop();
                    let the_cards = SavedCard::search_in_cards(&input, &cards, &excluded_cards);
                    print_stuff(&input, the_cards, &mut index);
                }
                KeyCode::Enter => {
                    let the_cards = SavedCard::search_in_cards(&input, &cards, &excluded_cards);
                    if the_cards.is_empty() {
                        return None;
                    }
                    return Some(the_cards[index].to_owned());
                }
                KeyCode::Down => {
                    index += 1;

                    let the_cards = SavedCard::search_in_cards(&input, &cards, &excluded_cards);
                    print_stuff(&input, the_cards, &mut index);
                }
                KeyCode::Up => {
                    let the_cards = SavedCard::search_in_cards(&input, &cards, &excluded_cards);
                    index = index.saturating_sub(1);
                    print_stuff(&input, the_cards, &mut index);
                }
                KeyCode::Esc => return None,
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

pub fn move_far_left(stdout: &mut Stdout) {
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
    cache: &mut CardCache,
) -> Option<SavedCard> {
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

pub fn add_cards(stdout: &mut Stdout, mut category: Category, cache: &mut CardCache) {
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

fn print_card_review_front(stdout: &mut Stdout, card: &Card, sound: bool) {
    execute!(stdout, MoveTo(0, 1)).unwrap();
    println!("{}", card.front.text);
    if sound {
        //  card.front.audio.play_audio();
    }
}

fn print_card_review_back(stdout: &mut Stdout, card: &Card, sound: bool) {
    move_far_left(stdout);
    execute!(stdout, MoveDown(1)).unwrap();
    move_far_left(stdout);
    println!("------------------");
    execute!(stdout, MoveDown(1)).unwrap();
    move_far_left(stdout);
    println!("{}", card.back.text);
    move_far_left(stdout);

    if sound {
        //card.back.audio.play_audio();
    }
}

fn should_exit(key: &KeyCode) -> bool {
    matches!(key, KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q'))
}

fn print_card_for_review(stdout: &mut Stdout, card: &SavedCard, show_backside: bool, status: &str) {
    execute!(stdout, Clear(ClearType::All)).unwrap();
    update_status_bar(stdout, status);
    print_card_review_front(stdout, card.card_as_ref(), true);
    if show_backside {
        print_card_review_back(stdout, card.card_as_ref(), true);
    }
}

fn review_card(
    stdout: &mut Stdout,
    card_id: &Id,
    status: String,
    cache: &mut CardCache,
) -> SomeStatus {
    let mut show_backside = false;
    loop {
        let card = cache.get_ref(card_id);
        print_card_for_review(stdout, &card, show_backside, status.as_str());
        let keycode = get_keycode();
        if edit_card(stdout, &keycode, card.clone(), cache) {
            continue;
        }
        match keycode {
            KeyCode::Char('o') => view_all_cards(stdout, cache),
            KeyCode::Char('X') => {
                let _ = ascii_test(stdout, card.id(), cache, true);
            }
            KeyCode::Char('x') => {
                let _ = ascii_test(stdout, card.id(), cache, false);
            }
            KeyCode::Char('Y') => {
                draw_message(stdout, "Adding new dependency");
                let category = Some(card.category().to_owned());
                add_dependency(stdout, card.id(), category.as_ref(), cache);
            }
            KeyCode::Char('T') => {
                draw_message(stdout, "Adding new dependent");
                let category = Some(card.category().to_owned());
                add_dependent(stdout, card.id(), category.as_ref(), cache);
            }
            KeyCode::Char('q') => return SomeStatus::Break,
            KeyCode::Char('D') => {
                if affirmative(stdout, "Delete card?") {
                    cache.get_owned(card.id()).delete(cache);
                    draw_message(stdout, "Card deleted");
                    break;
                }
            }
            KeyCode::Char(' ') => show_backside = true,
            KeyCode::Char('s') => break,
            KeyCode::Char('a') => {
                add_card(stdout, &mut card.category().to_owned(), cache);
            }
            KeyCode::Char(c) if show_backside => match c.to_string().parse() {
                Ok(grade) => {
                    cache.get_owned(card_id).new_review(grade);
                    return SomeStatus::Continue;
                }
                _ => continue,
            },
            key if should_exit(&key) => return SomeStatus::Break,
            _ => continue,
        }
    }
    SomeStatus::Continue
}

/// Bool represents if any action was taken.
pub fn edit_card(
    stdout: &mut Stdout,
    key: &KeyCode,
    card: Arc<SavedCard>,
    cache: &mut CardCache,
) -> bool {
    let mut excluded_cards = HashSet::new();
    excluded_cards.insert(card.id().to_owned());
    match key {
        KeyCode::Char('`') => {
            let info = format!("{:?}", card.get_info(cache));
            draw_message(stdout, info.as_str());
        }
        KeyCode::Char('p') => {
            let ch = _get_char();
            if let Ok(priority) = ch.try_into() {
                cache.get_owned(card.id()).set_priority(priority);
            }
        }

        KeyCode::Char('P') => {
            draw_message(stdout, "choose priority, from 0 to 100");
            if let Some(input) = read_user_input(stdout) {
                if let Ok(num) = input.0.trim().parse::<u32>() {
                    let priority: Priority = num.into();
                    cache.get_owned(card.id()).set_priority(priority);
                }
            }
        }

        KeyCode::Char('f') => {
            let mut thecard = cache.get_owned(card.id());
            thecard.set_finished(true);
        }

        KeyCode::Char('S') => suspend_card(stdout, card.id(), cache),

        KeyCode::Char('g') => {
            let tags = card.category().get_tags().into_iter().collect();
            let tag = match pick_item(stdout, "Choose tag", &tags) {
                Some(tag) => tag,
                None => return true,
            };
            let mut thecard = cache.get_owned(card.id());
            thecard.insert_tag(tag.to_owned());
        }

        KeyCode::Char('y') => {
            if let Some(chosen_card) = search_for_item(stdout, "Add dependency", excluded_cards) {
                cache
                    .get_owned(card.id())
                    .set_dependency(chosen_card.id(), cache);
                cache.refresh();
            }
        }
        KeyCode::Char('t') => {
            if let Some(chosen_card) = search_for_item(stdout, "Add dependent", excluded_cards) {
                let info = cache
                    .get_owned(card.id())
                    .set_dependent(chosen_card.id(), cache);
                if let Some(info) = info {
                    draw_message(stdout, &info);
                }
            }
        }
        KeyCode::Char('v') => {
            view_dependencies(stdout, card.id(), cache);
        }
        KeyCode::Char('m') => {
            let folder = match choose_folder(stdout, "Move card to...") {
                Some(folder) => folder,
                None => return true,
            };

            let moved_card = cache.get_owned(card.id()).move_card(&folder, cache);
            cache.insert(moved_card);
        }
        KeyCode::Char('e') => {
            card.edit_with_vim();
        }
        _ => return false,
    };
    true
}

fn view_cards(stdout: &mut Stdout, mut cards: Vec<Id>, cache: &mut CardCache) {
    if cards.is_empty() {
        draw_message(stdout, "No cards found");
        return;
    }

    let mut selected = 0;

    loop {
        let card_qty = cards.len();
        let card = cache.get_ref(&cards[selected]);
        let mut excluded_cards = HashSet::new();
        excluded_cards.insert(card.id().to_owned());

        let message = format!(
            "{}/{}\t{}\n{}\n-------------------\n{}",
            selected + 1,
            card_qty,
            card.category().print_full(),
            card.front_text(),
            card.back_text()
        );

        let key_event = draw_key_event_message(stdout, &message);

        if edit_card(stdout, &key_event.code, card.clone(), cache) {
            continue;
        }

        match key_event.code {
            KeyCode::Char('l') | KeyCode::Right if selected != card_qty - 1 => selected += 1,
            KeyCode::Char('h') | KeyCode::Left if selected != 0 => selected -= 1,
            KeyCode::Char('.') => panic!(),
            KeyCode::Char('X') => {
                if let Some(thecard) = ascii_test(stdout, card.id(), cache, true) {
                    let mut idx = None;
                    for card in cards.iter().enumerate() {
                        if card.1 == thecard.id() {
                            idx = Some(card.0);
                        }
                    }

                    if let Some(idx) = idx {
                        cards.swap(0, idx);
                        selected = 0;
                    } else {
                        draw_message(stdout, "damn ...");
                    }
                }
            }

            KeyCode::Char('x') => {
                if let Some(thecard) = ascii_test(stdout, card.id(), cache, false) {
                    let mut idx = None;
                    for card in cards.iter().enumerate() {
                        if card.1 == thecard.id() {
                            idx = Some(card.0);
                        }
                    }

                    if let Some(idx) = idx {
                        cards.swap(0, idx);
                        selected = 0;
                    } else {
                        draw_message(stdout, "damn ...");
                    }
                }
            }

            KeyCode::Char('T') => {
                draw_message(stdout, "Adding new dependent");
                let category = Some(card.category().to_owned());
                if let Some(updated_card) =
                    add_dependent(stdout, card.id(), category.as_ref(), cache)
                {
                    cards.insert(0, *updated_card.id());
                }
            }
            KeyCode::Char('Y') => {
                draw_message(stdout, "Adding new dependency");
                let category = Some(card.category().to_owned());
                if let Some(updated_card) =
                    add_dependency(stdout, card.id(), category.as_ref(), cache)
                {
                    cards.insert(0, *updated_card.id());
                }
            }

            KeyCode::Char('a') => {
                if let Some(card) = add_card(stdout, &mut card.category().clone(), cache) {
                    cards.insert(0, card.id().to_owned()); // temp thing
                }
            }
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
            KeyCode::Char('D') => {
                cache.get_owned(card.id()).delete(cache);
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
            KeyCode::Char('/') => {
                if let Some(thecard) = search_for_item(stdout, "find some card", excluded_cards) {
                    let mut idx = None;
                    for card in cards.iter().enumerate() {
                        if card.1 == thecard.id() {
                            idx = Some(card.0);
                        }
                    }

                    if let Some(idx) = idx {
                        cards.swap(0, idx);
                        selected = 0;
                    } else {
                        draw_message(stdout, "damn ...");
                    }
                }
            }
            key if should_exit(&key) => return,
            _ => {}
        };
    }
}

fn get_following_unfinished_cards(category: &Category, cache: &mut CardCache) -> Vec<Id> {
    let categories = category.get_following_categories();
    let mut cards = vec![];
    for category in categories {
        cards.extend(category.get_unfinished_cards(cache));
    }
    randvec(cards)
}

pub type CardsFromCategory = Box<dyn FnMut(&Category, &mut CardCache) -> Vec<Id>>;

pub fn review_cards(
    stdout: &mut Stdout,
    category: Category,
    mut get_cards: CardsFromCategory,
    cache: &mut CardCache,
) {
    let categories = category.get_following_categories();
    let mut cards = BTreeSet::new();
    for category in &categories {
        cards.extend(get_cards(category, cache));
    }

    let mut cards: Vec<Id> = cards.into_iter().collect();
    cards.sort_by_key(|card| {
        (cache.get_ref(card).expected_gain().unwrap_or_default() * 1000.) as i32
    });
    cards.reverse();

    let cardqty = cards.len();

    for (index, card) in cards.into_iter().enumerate() {
        let info = cache.get_ref(&card).get_info(cache).unwrap_or_default();
        let status = format!(
            "{}/{}\t{}\t{}/{}/{}/{}/{}",
            index,
            cardqty,
            cache.get_ref(&card).category().print_full(),
            cache.dependencies(&card).len(),
            cache.dependents(&card).len(),
            (info.recall_rate * 100.).round(),
            (info.stability * 100.).round() / 100.,
            info.strength.round(),
        );
        match {
            match cache.get_ref(&card).get_review_type() {
                ReviewType::Normal | ReviewType::Pending => {
                    review_card(stdout, &card, status.clone(), cache)
                }

                ReviewType::Unfinished => continue,
            }
        } {
            SomeStatus::Continue => {
                continue;
            }
            SomeStatus::Break => return,
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

pub fn _get_char() -> char {
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

fn draw_menu(
    stdout: &mut Stdout,
    message: Option<&str>,
    items: Vec<&str>,
    optional: bool,
) -> Option<usize> {
    let mut selected = 0;

    loop {
        execute!(stdout, Clear(ClearType::All)).unwrap();
        move_upper_left(stdout);
        if let Some(message) = message {
            println!("{message}");
        }

        for (index, item) in items.iter().enumerate() {
            execute!(stdout, MoveTo(0, index as u16 + 1)).unwrap();

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

pub fn add_dependency(
    stdout: &mut Stdout,
    card: &Id,
    category: Option<&Category>,
    cache: &mut CardCache,
) -> Option<SavedCard> {
    let mut card = cache.get_owned(card);
    let category = category.unwrap_or_else(|| card.category());
    let category = &mut category.to_owned();
    let new_dependency = add_card(stdout, category, cache)?;
    let info = card.set_dependency(new_dependency.id(), cache);

    if let Some(info) = info {
        draw_message(stdout, &info);
    }
    cache.refresh();
    Some(new_dependency)
}

pub fn add_dependent(
    stdout: &mut Stdout,
    card: &Id,
    category: Option<&Category>,
    cache: &mut CardCache,
) -> Option<SavedCard> {
    let mut card = cache.get_owned(card);
    let mut category = category.cloned().unwrap_or_else(|| card.category().clone());
    let new_dependent = add_card(stdout, &mut category, cache)?;
    let info = card.set_dependent(new_dependent.id(), cache);

    if let Some(info) = info {
        draw_message(stdout, &info);
    }
    cache.refresh();
    Some(new_dependent)
}
