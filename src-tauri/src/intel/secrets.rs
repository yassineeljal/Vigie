//! Stockage de la clé API HIBP dans le Trousseau macOS (jamais dans la base ni les réglages).

use secrecy::SecretString;

use super::IntelError;

pub trait SecretStore: Send + Sync {
    fn get(&self, name: &str) -> Result<Option<SecretString>, IntelError>;
    fn set(&self, name: &str, value: &SecretString) -> Result<(), IntelError>;
    fn delete(&self, name: &str) -> Result<(), IntelError>;
}

const SERVICE: &str = "ca.vigie.app";

#[derive(Default)]
pub struct KeychainStore;

#[cfg(target_os = "macos")]
impl SecretStore for KeychainStore {
    fn get(&self, name: &str) -> Result<Option<SecretString>, IntelError> {
        use security_framework::passwords::get_generic_password;
        match get_generic_password(SERVICE, name) {
            Ok(bytes) => Ok(String::from_utf8(bytes).ok().map(SecretString::from)),
            // errSecItemNotFound
            Err(e) if e.code() == -25300 => Ok(None),
            Err(e) => Err(IntelError::Keychain(e.to_string())),
        }
    }
    fn set(&self, name: &str, value: &SecretString) -> Result<(), IntelError> {
        use secrecy::ExposeSecret;
        use security_framework::passwords::set_generic_password;
        set_generic_password(SERVICE, name, value.expose_secret().as_bytes())
            .map_err(|e| IntelError::Keychain(e.to_string()))
    }
    fn delete(&self, name: &str) -> Result<(), IntelError> {
        use security_framework::passwords::delete_generic_password;
        match delete_generic_password(SERVICE, name) {
            Ok(()) => Ok(()),
            Err(e) if e.code() == -25300 => Ok(()),
            Err(e) => Err(IntelError::Keychain(e.to_string())),
        }
    }
}

#[cfg(not(target_os = "macos"))]
impl SecretStore for KeychainStore {
    fn get(&self, _: &str) -> Result<Option<SecretString>, IntelError> {
        Err(IntelError::Keychain(
            "non supporté sur cette plateforme".into(),
        ))
    }
    fn set(&self, _: &str, _: &SecretString) -> Result<(), IntelError> {
        Err(IntelError::Keychain(
            "non supporté sur cette plateforme".into(),
        ))
    }
    fn delete(&self, _: &str) -> Result<(), IntelError> {
        Ok(())
    }
}

#[cfg(test)]
#[derive(Default)]
pub struct MemoryStore(std::sync::Mutex<std::collections::HashMap<String, String>>);

#[cfg(test)]
impl SecretStore for MemoryStore {
    fn get(&self, name: &str) -> Result<Option<SecretString>, IntelError> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .get(name)
            .cloned()
            .map(SecretString::from))
    }
    fn set(&self, name: &str, value: &SecretString) -> Result<(), IntelError> {
        use secrecy::ExposeSecret;
        self.0
            .lock()
            .unwrap()
            .insert(name.into(), value.expose_secret().into());
        Ok(())
    }
    fn delete(&self, name: &str) -> Result<(), IntelError> {
        self.0.lock().unwrap().remove(name);
        Ok(())
    }
}
