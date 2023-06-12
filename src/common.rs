use crate::card::Card;
use crate::folders::get_cards_from_category;
use crate::{paths::get_cards_path, Id};
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

pub fn current_time() -> Duration {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("Time went backwards")
}

// Represent the category that a card is in, can be nested
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Category(pub Vec<String>);

impl Category {
    pub fn sort_categories(categories: &mut [Category]) {
        categories.sort_by(|a, b| {
            let a_str = a.0.join("/");
            let b_str = b.0.join("/");
            a_str.cmp(&b_str)
        });
    }

    pub fn get_containing_cards(&self) -> Vec<Card> {
        get_cards_from_category(self)
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

    pub fn _from_card_path(path: &Path) -> Self {
        let root = get_cards_path();
        let path = path.strip_prefix(root).unwrap();

        let mut x = path
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<String>>();
        x.pop();
        Category(x)
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
    pub fn as_path_with_id(&self, id: Id) -> PathBuf {
        let mut folder = self.as_path();
        folder = folder.join(id.to_string());
        folder.set_extension("toml");
        folder
    }

    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }
}

pub mod serde_duration_as_secs {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let secs = duration.as_secs();
        serializer.serialize_u64(secs)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(deserializer)?;
        Ok(Duration::from_secs(secs))
    }
}

#[cfg(test)]
mod tests {

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
