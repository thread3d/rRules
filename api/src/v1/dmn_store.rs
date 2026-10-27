//! Which backend the decision endpoints persist through.
//!
//! `FileSystemDmnStore` is the default and the authoritative store: it writes the `.dmn` XML a
//! deployment's `DMN_STORAGE_PATH` holds and git tracks. `PostgresDmnStore` is the alternative for
//! deployments that keep decision documents in a database; it is compiled only with the `postgres`
//! feature, and selected with `DMN_PERSISTENCE=postgres` plus a `DATABASE_URL`.
//!
//! The selection is deliberately explicit and fails loudly: asking for `postgres` when the feature is
//! not compiled, or when `DATABASE_URL` is missing, is a startup error. Silently falling back to the
//! filesystem would mean a deployment believing its documents are in a database that never received
//! them — the same class of lie as a store whose methods return `NotConnected`.

use engine_state::{DmnDocument, DmnPersistence, DmnSummary, FileSystemDmnStore, PersistenceError};

/// The store the decision handlers write through.
pub enum DmnStore {
    /// `.dmn` files under `DMN_STORAGE_PATH`: the default, and what git tracks.
    FileSystem(FileSystemDmnStore),
    /// A PostgreSQL database reached through `DATABASE_URL`.
    #[cfg(feature = "postgres")]
    Postgres(engine_state::postgres::PostgresDmnStore),
}

impl DmnStore {
    /// The filesystem store, rooted at `DMN_STORAGE_PATH` (or `./dmn`).
    pub fn from_filesystem() -> Self {
        Self::FileSystem(FileSystemDmnStore::default())
    }

    /// Choose the backend from `DMN_PERSISTENCE` (`filesystem`, the default, or `postgres`).
    ///
    /// # Errors
    /// Returns an error when `postgres` is requested but the feature is not compiled, when
    /// `DATABASE_URL` is unset, when the connection fails, or when `DMN_PERSISTENCE` names something
    /// this build does not implement.
    pub async fn from_environment() -> Result<Self, String> {
        let requested =
            std::env::var("DMN_PERSISTENCE").unwrap_or_else(|_| "filesystem".to_string());
        match requested.as_str() {
            "filesystem" | "" => Ok(Self::from_filesystem()),
            "postgres" => {
                #[cfg(feature = "postgres")]
                {
                    let store = engine_state::postgres::PostgresDmnStore::from_env()
                        .await
                        .map_err(|error| {
                            format!("DMN_PERSISTENCE=postgres, but the store could not connect: {error}")
                        })?;
                    Ok(Self::Postgres(store))
                }
                #[cfg(not(feature = "postgres"))]
                {
                    Err("DMN_PERSISTENCE=postgres, but this build has no `postgres` feature; \
                         rebuild with --features postgres rather than silently using the filesystem"
                        .to_string())
                }
            }
            other => Err(format!(
                "DMN_PERSISTENCE={other} is not a store this build implements; use `filesystem` or `postgres`"
            )),
        }
    }

    /// The backend's name, for logs and `/v1/decisions` metadata.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::FileSystem(_) => "filesystem",
            #[cfg(feature = "postgres")]
            Self::Postgres(_) => "postgres",
        }
    }
}

impl DmnPersistence for DmnStore {
    async fn list(&self) -> Result<Vec<DmnSummary>, PersistenceError> {
        match self {
            Self::FileSystem(store) => store.list().await,
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => store.list().await,
        }
    }

    async fn get(&self, id: &str) -> Result<DmnDocument, PersistenceError> {
        match self {
            Self::FileSystem(store) => store.get(id).await,
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => store.get(id).await,
        }
    }

    async fn create(&self, doc: DmnDocument) -> Result<DmnDocument, PersistenceError> {
        match self {
            Self::FileSystem(store) => store.create(doc).await,
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => store.create(doc).await,
        }
    }

    async fn update(&self, id: &str, doc: DmnDocument) -> Result<DmnDocument, PersistenceError> {
        match self {
            Self::FileSystem(store) => store.update(id, doc).await,
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => store.update(id, doc).await,
        }
    }

    async fn delete(&self, id: &str) -> Result<(), PersistenceError> {
        match self {
            Self::FileSystem(store) => store.delete(id).await,
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => store.delete(id).await,
        }
    }

    async fn save_file(&self, filename: &str, content: &[u8]) -> Result<(), PersistenceError> {
        match self {
            Self::FileSystem(store) => store.save_file(filename, content).await,
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => store.save_file(filename, content).await,
        }
    }

    async fn load_file(&self, filename: &str) -> Result<Vec<u8>, PersistenceError> {
        match self {
            Self::FileSystem(store) => store.load_file(filename).await,
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => store.load_file(filename).await,
        }
    }
}
