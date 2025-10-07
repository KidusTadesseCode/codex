use ignore::gitignore::Gitignore;
use ignore::gitignore::GitignoreBuilder;
use ignore::Match;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

/// Wrapper around the `ignore` crate that loads `.codexignore` patterns and
/// exposes convenience helpers for matching filesystem paths.
#[derive(Debug, Clone)]
pub struct CodexIgnore {
    root: PathBuf,
    matcher: Arc<Gitignore>,
}

impl CodexIgnore {
    /// Attempts to load `.codexignore` from `root`. Returns `Ok(None)` when the
    /// file does not exist.
    pub fn load_from_root(root: &Path) -> std::io::Result<Option<Self>> {
        let path = root.join(".codexignore");
        if !path.exists() {
            return Ok(None);
        }

        let mut builder = GitignoreBuilder::new(root);
        builder
            .add(path)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?;
        let matcher = builder
            .build()
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?;

        Ok(Some(Self {
            root: root.to_path_buf(),
            matcher: Arc::new(matcher),
        }))
    }

    /// Returns the project root used to resolve relative paths.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns `true` when the provided path (file) should be ignored.
    pub fn is_file_ignored(&self, path: &Path) -> bool {
        self.is_ignored(path, false)
    }

    /// Returns `true` when the provided directory path should be ignored.
    pub fn is_dir_ignored(&self, path: &Path) -> bool {
        self.is_ignored(path, true)
    }

    /// Returns the path relative to the codexignore root, if possible.
    pub fn relative_path<'a>(&self, path: &'a Path) -> Option<PathBuf> {
        let abs = self.to_absolute(path);
        abs.strip_prefix(&self.root).map(PathBuf::from).ok()
    }

    fn is_ignored(&self, path: &Path, is_dir: bool) -> bool {
        let abs = self.to_absolute(path);
        let Some(rel) = abs.strip_prefix(&self.root).ok() else {
            return false;
        };
        let rel = if rel.as_os_str().is_empty() {
            Path::new(".")
        } else {
            rel
        };

        match self.matcher.matched_path_or_any_parents(rel, is_dir) {
            Match::Ignore(_) => true,
            Match::Whitelist(_) | Match::None => false,
        }
    }

    fn to_absolute(&self, path: &Path) -> PathBuf {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root.join(path)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn returns_none_when_file_missing() {
        let tmp = TempDir::new().unwrap();
        let ignore = CodexIgnore::load_from_root(tmp.path()).unwrap();
        assert!(ignore.is_none());
    }

    #[test]
    fn matches_files_and_directories() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::write(
            root.join(".codexignore"),
            "ignored_dir/\nsecret.txt\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("ignored_dir")).unwrap();

        let ignore = CodexIgnore::load_from_root(root).unwrap().unwrap();
        assert!(ignore.is_dir_ignored(root.join("ignored_dir").as_path()));
        assert!(ignore.is_file_ignored(root.join("secret.txt").as_path()));
        assert!(!ignore.is_file_ignored(root.join("visible.txt").as_path()));
    }

    #[test]
    fn relative_path_handles_absolute_and_relative_inputs() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::write(root.join(".codexignore"), "secret.txt\n").unwrap();
        let ignore = CodexIgnore::load_from_root(root).unwrap().unwrap();

        let rel = ignore.relative_path(Path::new("secret.txt")).unwrap();
        assert_eq!(rel, PathBuf::from("secret.txt"));

        let abs = ignore
            .relative_path(&root.join("nested").join("file.txt"))
            .unwrap();
        assert_eq!(abs, PathBuf::from("nested").join("file.txt"));
    }
}
