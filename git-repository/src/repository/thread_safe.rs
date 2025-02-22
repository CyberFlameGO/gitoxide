mod access {
    use crate::Kind;

    impl crate::ThreadSafeRepository {
        /// Return the kind of repository, either bare or one with a work tree.
        pub fn kind(&self) -> Kind {
            match self.work_tree {
                Some(_) => Kind::WorkTree,
                None => Kind::Bare,
            }
        }

        /// Add thread-local state to an easy-to-use thread-local repository for the most convenient API.
        pub fn to_thread_local(&self) -> crate::Repository {
            self.into()
        }
    }
}

mod from_path {
    use std::convert::TryFrom;

    use crate::Path;

    impl TryFrom<crate::Path> for crate::ThreadSafeRepository {
        type Error = crate::open::Error;

        fn try_from(value: Path) -> Result<Self, Self::Error> {
            let (git_dir, worktree_dir) = value.into_repository_and_work_tree_directories();
            crate::ThreadSafeRepository::open_from_paths(git_dir, worktree_dir, Default::default())
        }
    }
}

mod location {

    impl crate::ThreadSafeRepository {
        /// The path to the `.git` directory itself, or equivalent if this is a bare repository.
        pub fn path(&self) -> &std::path::Path {
            self.git_dir()
        }

        /// Return the path to the repository itself, containing objects, references, configuration, and more.
        ///
        /// Synonymous to [`path()`][crate::ThreadSafeRepository::path()].
        pub fn git_dir(&self) -> &std::path::Path {
            self.refs.base()
        }

        /// Return the path to the working directory if this is not a bare repository.
        pub fn workdir(&self) -> Option<&std::path::Path> {
            self.work_tree.as_deref()
        }

        /// Return the path to the directory containing all objects.
        pub fn objects_dir(&self) -> &std::path::Path {
            self.objects.path()
        }
    }
}

mod impls {
    impl std::fmt::Debug for crate::ThreadSafeRepository {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                f,
                "Repository(git = '{}', working_tree: {:?}",
                self.git_dir().display(),
                self.work_tree
            )
        }
    }

    impl PartialEq<crate::ThreadSafeRepository> for crate::ThreadSafeRepository {
        fn eq(&self, other: &crate::ThreadSafeRepository) -> bool {
            self.git_dir() == other.git_dir() && self.work_tree == other.work_tree
        }
    }
}
