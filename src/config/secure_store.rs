use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use serde::{Serialize, de::DeserializeOwned};

const SERVICE_NAME: &str = "soundcloud-tui";

pub const CREDENTIALS_ENTRY: &str = "oauth-credentials";
pub const TOKENS_ENTRY: &str = "oauth-tokens";

pub fn troubleshooting_hint(error: &str) -> Option<&'static str> {
    if !cfg!(target_os = "linux") {
        return None;
    }

    let error = error.to_ascii_lowercase();
    if error.contains("org.freedesktop.secrets")
        || error.contains("the name is not activatable")
        || error.contains("secret service")
        || error.contains("secret collection")
        || error.contains("default collection")
        || error.contains("dbus")
    {
        Some(
            "Linux tip: install and start gnome-keyring, then log into a fresh graphical session so Secret Service is available.",
        )
    } else {
        None
    }
}

pub fn load_secret<T>(entry_name: &str, label: &str) -> Result<Option<T>>
where
    T: DeserializeOwned,
{
    let Some(raw) = backend()
        .load(entry_name)
        .with_context(|| format!("Could not access {label} in your OS keyring"))?
    else {
        return Ok(None);
    };

    serde_json::from_str(&raw)
        .map(Some)
        .map_err(|error| anyhow!("invalid {label} stored in your OS keyring: {error}"))
}

pub fn save_secret<T>(entry_name: &str, label: &str, value: &T) -> Result<()>
where
    T: Serialize,
{
    let raw = serde_json::to_string(value)?;
    backend()
        .save(entry_name, &raw)
        .with_context(|| format!("Could not save {label} to your OS keyring"))
}

pub fn delete_secret(entry_name: &str, label: &str) -> Result<()> {
    backend()
        .delete(entry_name)
        .with_context(|| format!("Could not remove {label} from your OS keyring"))
}

pub(crate) trait SecretBackend: Send + Sync {
    fn load(&self, entry_name: &str) -> Result<Option<String>>;
    fn save(&self, entry_name: &str, value: &str) -> Result<()>;
    fn delete(&self, entry_name: &str) -> Result<()>;
}

struct OsKeyringBackend;

impl SecretBackend for OsKeyringBackend {
    fn load(&self, entry_name: &str) -> Result<Option<String>> {
        let entry = keyring::Entry::new(SERVICE_NAME, entry_name)?;
        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn save(&self, entry_name: &str, value: &str) -> Result<()> {
        let entry = keyring::Entry::new(SERVICE_NAME, entry_name)?;
        entry.set_password(value)?;
        Ok(())
    }

    fn delete(&self, entry_name: &str) -> Result<()> {
        let entry = keyring::Entry::new(SERVICE_NAME, entry_name)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

fn backend() -> Arc<dyn SecretBackend> {
    #[cfg(test)]
    {
        if let Some(backend) = test_backend::get() {
            return backend;
        }
    }

    Arc::new(OsKeyringBackend)
}

#[cfg(test)]
mod test_backend {
    use std::{
        collections::HashMap,
        panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
        sync::{Arc, Mutex, OnceLock},
    };

    use anyhow::{Result, bail};

    use super::SecretBackend;

    static BACKEND: OnceLock<Mutex<Option<Arc<dyn SecretBackend>>>> = OnceLock::new();
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    pub(crate) fn get() -> Option<Arc<dyn SecretBackend>> {
        BACKEND
            .get_or_init(|| Mutex::new(None))
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    pub(crate) fn with_test_backend<T>(
        backend: Arc<dyn SecretBackend>,
        run: impl FnOnce() -> T,
    ) -> T {
        let _guard = LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        *BACKEND
            .get_or_init(|| Mutex::new(None))
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(backend);
        let result = catch_unwind(AssertUnwindSafe(run));
        *BACKEND
            .get_or_init(|| Mutex::new(None))
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = None;
        match result {
            Ok(result) => result,
            Err(payload) => resume_unwind(payload),
        }
    }

    #[derive(Default)]
    pub(crate) struct MemoryBackend {
        entries: Mutex<HashMap<String, String>>,
        load_error: Mutex<Option<String>>,
        save_error: Mutex<Option<String>>,
        delete_error: Mutex<Option<String>>,
    }

    impl MemoryBackend {
        pub(crate) fn with_entry(self, entry_name: &str, value: &str) -> Self {
            self.entries
                .lock()
                .expect("entries")
                .insert(entry_name.to_string(), value.to_string());
            self
        }

        pub(crate) fn fail_load(self, message: &str) -> Self {
            *self.load_error.lock().expect("load error") = Some(message.to_string());
            self
        }

        pub(crate) fn fail_save(self, message: &str) -> Self {
            *self.save_error.lock().expect("save error") = Some(message.to_string());
            self
        }

        pub(crate) fn fail_delete(self, message: &str) -> Self {
            *self.delete_error.lock().expect("delete error") = Some(message.to_string());
            self
        }
    }

    impl SecretBackend for MemoryBackend {
        fn load(&self, entry_name: &str) -> Result<Option<String>> {
            if let Some(message) = self.load_error.lock().expect("load error").clone() {
                bail!(message);
            }

            Ok(self
                .entries
                .lock()
                .expect("entries")
                .get(entry_name)
                .cloned())
        }

        fn save(&self, entry_name: &str, value: &str) -> Result<()> {
            if let Some(message) = self.save_error.lock().expect("save error").clone() {
                bail!(message);
            }

            self.entries
                .lock()
                .expect("entries")
                .insert(entry_name.to_string(), value.to_string());
            Ok(())
        }

        fn delete(&self, entry_name: &str) -> Result<()> {
            if let Some(message) = self.delete_error.lock().expect("delete error").clone() {
                bail!(message);
            }

            self.entries.lock().expect("entries").remove(entry_name);
            Ok(())
        }
    }
}

#[cfg(test)]
pub(crate) use test_backend::{MemoryBackend, with_test_backend};

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde::{Deserialize, Serialize};

    use super::{
        CREDENTIALS_ENTRY, MemoryBackend, delete_secret, load_secret, save_secret,
        troubleshooting_hint, with_test_backend,
    };

    #[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
    struct SampleSecret {
        value: String,
    }

    #[test]
    fn secret_round_trip_uses_configured_backend() {
        with_test_backend(Arc::new(MemoryBackend::default()), || {
            let sample = SampleSecret {
                value: "secret-value".to_string(),
            };

            save_secret(CREDENTIALS_ENTRY, "test secret", &sample).expect("save secret");

            let loaded = load_secret::<SampleSecret>(CREDENTIALS_ENTRY, "test secret")
                .expect("load secret")
                .expect("stored secret");

            assert_eq!(loaded, sample);
        });
    }

    #[test]
    fn delete_secret_removes_existing_value() {
        let backend =
            MemoryBackend::default().with_entry(CREDENTIALS_ENTRY, r#"{"value":"secret-value"}"#);

        with_test_backend(Arc::new(backend), || {
            delete_secret(CREDENTIALS_ENTRY, "test secret").expect("delete secret");
            let loaded =
                load_secret::<SampleSecret>(CREDENTIALS_ENTRY, "test secret").expect("load secret");
            assert!(loaded.is_none());
        });
    }

    #[test]
    fn load_secret_surfaces_keyring_access_errors() {
        let backend = MemoryBackend::default().fail_load("keyring locked");

        with_test_backend(Arc::new(backend), || {
            let error = load_secret::<SampleSecret>(CREDENTIALS_ENTRY, "test secret")
                .expect_err("expected load failure");
            assert!(error.to_string().contains("OS keyring"));
            assert!(format!("{error:#}").contains("keyring locked"));
        });
    }

    #[test]
    fn troubleshooting_hint_detects_missing_secret_service() {
        let hint = troubleshooting_hint("The name org.freedesktop.secrets was not provided")
            .expect("linux hint");

        assert!(hint.contains("gnome-keyring"));
    }
}
