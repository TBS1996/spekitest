use rusqlite::{params, Connection, Result, NO_PARAMS};



use crate::card::Card;
use crate::common::{current_time, Category};
use crate::folders::{get_category_from_id_from_fs, get_last_modified_map_from_category};
use crate::{get_cards_path, Conn, Id};

pub struct CardMetaData {
    file_name: String,
    category: Category,
    last_modified: u64,
    file_size: u64,
}

fn sync_my_shit(conn: &Conn) {
    let categories = Category::load_all().unwrap();

    for category in &categories {
        let _category_path = category.as_path();
        let _cards = get_last_modified_map_from_category(conn, category);
    }
}

/*


remember: SPOT! :D
    sync plan:

    iterate over all the folders.
    for each folder, grab the full category name, e.g. "math/calculus"
    make a hashmap on each folder, from the cache, where the key is all the names and the value is the corresponding last_modified date.



Cache plan:

when fetching a card with an ID:


1. look up the path in the cache
    if the path is saved, check if it exists
        if the path doesn't point to a valid, search for the card
            if you find it, update the cache
        else:
            if you don't find it, return a None value all the way up
    if the path isn't




 */

pub fn get_cached_path_from_db(id: Id, conn: &Conn) -> Option<Category> {
    let category: Option<String> = conn
        .query_row(
            "SELECT path FROM cards WHERE id = ?1",
            params![id.to_string()],
            |row| row.get(0),
        )
        .ok();
    let category = Category::from_string(category?);

    if category.as_path_with_id(id).exists() {
        return Some(category);
    }

    if let Some(category) = get_category_from_id_from_fs(id) {
        update_category(id, &category, conn);
        return Some(category);
    }

    // Cache is invalid, so delete.
    delete_card_cache(conn, id).unwrap();
    None
}

fn delete_card_cache(conn: &Connection, id: Id) -> Result<()> {
    conn.execute("DELETE FROM cards WHERE id = ?1", params![id.to_string()])?;
    Ok(())
}

pub fn delete_the_card_cache(conn: &Connection, id: Id) {
    conn.execute("DELETE FROM cards WHERE id = ?1", params![id.to_string()])
        .unwrap();

    conn.execute(
        "DELETE FROM strength WHERE id = ?1",
        params![id.to_string()],
    )
    .unwrap();
}

pub fn index_cards(conn: &Conn) {
    let dir = &get_cards_path();

    Card::process_cards(dir, &mut |_card: Card, _path: &Category| {
        let _conn = conn;
        //index_strength(conn, &card);
        //index_card(conn, &card, path);
        Ok(())
    })
    .unwrap();
}

pub fn cache_card_from_id(conn: &Conn, id: Id) {
    let _card = Card::load_from_id(id, conn).unwrap();
}

pub fn cache_card(conn: &Conn, card: &Card, category: &Category) {
    let id = card.meta.id.to_string();
    let front = card.front.text.to_owned();
    let back = card.back.text.to_owned();
    let category = category.joined();
    let strength = card.calculate_strength();
    let last_calc = current_time().as_secs() as i64;

    conn.execute(
        "INSERT OR REPLACE INTO cards (id, front, back, category, strength, last_calc) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, front, back, category, strength, last_calc],
    )
    .unwrap();
}

fn update_category(id: Id, category: &Category, conn: &Conn) {
    let new_category = category.joined();
    let id = id.to_string();
    conn.execute(
        "UPDATE cards SET category = ?1 WHERE id = ?2",
        params![new_category, id],
    )
    .unwrap();
}

pub fn init() -> Result<Conn> {
    let conn = Connection::open("cache.db")?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS cards (
            id TEXT PRIMARY KEY,
            front TEXT,
            back TEXT,
            category TEXT NOT NULL,
            strength FLOAT NOT NULL,
            last_calc INTEGER NOT NULL,
            last_modified INTEGER NOT NULL
        )",
        NO_PARAMS,
    )?;

    Ok(conn)
}
