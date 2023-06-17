use crate::card::AnnoCard;
use crate::folders::{get_all_cards_full, get_cards_from_category};
use crate::paths::{self, get_cards_path};
use crate::Id;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

// Represent the category that a card is in, can be nested
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Category(pub Vec<String>);

impl Category {
    pub fn from_card_path(path: &Path) -> Self {
        let without_prefix = path.strip_prefix(paths::get_cards_path()).unwrap();
        let folder = without_prefix.parent().unwrap();

        let components: Vec<String> = Path::new(folder)
            .components()
            .filter_map(|component| component.as_os_str().to_str().map(String::from))
            .collect();

        let categories = Self(components);

        if categories.as_path().exists() {
            categories
        } else {
            panic!();
        }
    }

    pub fn sort_categories(categories: &mut [Category]) {
        categories.sort_by(|a, b| {
            let a_str = a.0.join("/");
            let b_str = b.0.join("/");
            a_str.cmp(&b_str)
        });
    }
    pub fn get_following_categories(&self) -> Vec<Self> {
        let categories = Category::load_all().unwrap();
        let catlen = self.0.len();
        categories
            .into_iter()
            .filter(|cat| cat.0.len() >= catlen && cat.0[0..catlen] == self.0[0..catlen])
            .collect()
    }

    pub fn create(&self) {
        let path = self.as_path();
        std::fs::create_dir_all(path).unwrap();
    }

    pub fn print_it(&self) -> String {
        self.0.last().unwrap_or(&"root".to_string()).clone()
    }

    pub fn print_it_with_depth(&self) -> String {
        let mut s = String::new();
        for _ in 0..self.0.len() {
            s.push_str("  ");
        }
        format!("{}{}", s, self.print_it())
    }

    pub fn import_category() -> Self {
        Self(vec!["imports".into()])
    }

    pub fn load_all() -> io::Result<Vec<Category>> {
        let root = get_cards_path();
        let root = root.as_path();
        let mut folders = Vec::new();
        Self::collect_folders_inner(root, root, &mut folders)?;
        folders.push(Category::default());
        Ok(folders)
    }

    pub fn _append(mut self, category: &str) -> Self {
        self.0.push(category.into());
        self
    }

    fn collect_folders_inner(
        root: &Path,
        current: &Path,
        folders: &mut Vec<Category>,
    ) -> io::Result<()> {
        if current.is_dir() {
            for entry in fs::read_dir(current)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    // Compute the relative path from root to the current directory
                    let rel_path = path
                        .strip_prefix(root)
                        .expect("Failed to compute relative path")
                        .components()
                        .map(|c| c.as_os_str().to_string_lossy().into_owned())
                        .collect::<Vec<String>>();
                    folders.push(Self(rel_path));
                    Self::collect_folders_inner(root, &path, folders)?;
                }
            }
        }
        Ok(())
    }

    pub fn joined(&self) -> String {
        self.0.join("/")
    }

    pub fn from_string(s: String) -> Self {
        let vec = s.split('/').map(|s| s.to_string()).collect();
        Self(vec)
    }

    pub fn as_path(&self) -> PathBuf {
        let categories = self.0.join("/");
        let path = format!("{}/{}", get_cards_path().to_string_lossy(), categories);
        PathBuf::from(path)
    }

    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }

    pub fn get_unfinished_cards(&self) -> Vec<AnnoCard> {
        get_cards_from_category(self)
            .into_iter()
            .filter(|card| card.card_as_ref().is_ready_for_unfinished_review())
            .collect()
    }

    pub fn get_pending_cards(&self) -> Vec<AnnoCard> {
        let cards = get_cards_from_category(self);
        cards
            .into_iter()
            .filter(|card| card.card_as_ref().is_ready_for_pending_review())
            .collect()
    }

    pub fn get_review_cards(&self) -> Vec<AnnoCard> {
        let cards = get_cards_from_category(self);
        cards
            .into_iter()
            .filter(|card| card.card_as_ref().is_ready_for_review())
            .collect()
    }

    pub fn from_id(id: Id) -> Option<Self> {
        let cards = get_all_cards_full();
        for card in cards {
            if card.0.meta.id == id {
                return Some(card.1.category);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {

    use crate::categories::Category;

    use super::*;

    #[test]
    fn test_load_all() {
        let root = Path::new("./testing");
        let mut folders = vec![];
        Category::collect_folders_inner(root, root, &mut folders).unwrap();

        insta::assert_debug_snapshot!(folders);
    }

    #[test]
    fn test_joined() {
        let category = Category(vec!["foo".into(), "bar".into()]);
        let joined = category.joined();
        insta::assert_debug_snapshot!(joined);
    }

    #[test]
    fn test_as_card_path() {
        let cards_path = paths::get_cards_path()
            .join("foo")
            .join("bar")
            .join("guten tag.toml");
        Category::from_card_path(cards_path.as_path());
    }

    /*
    #[test]
    fn test_from_card_path() {
        let card_path = "./testing/maths/calculus/491f8b92-c943-4c4b-b7bf-f7d483208eb0.toml";
        let card_path = Path::new(card_path);
        let x = Category::from_card_path(card_path);
        insta::assert_debug_snapshot!(x);
    }

    #[test]
    fn test_as_path() {
        let category = Category(vec!["foo".into(), "bar".into()]);
        let x = category.as_path();
        insta::assert_debug_snapshot!(x);
    }

    #[test]
    fn test_as_path_with_id() {
        let id = uuid!("8bc35fe2-f02b-4633-8f1b-306eb4e09cd2");
        let category = Category(vec!["foo".into(), "bar".into()]);
        let x = category.as_path_with_id(id);
        insta::assert_debug_snapshot!(x);
    }
    */
}
