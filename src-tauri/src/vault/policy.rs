use secrecy::{ExposeSecret, SecretString};

use super::VaultError;

pub const MIN_SCORE: u8 = 3;

/// Exige un mot de passe maître d'un score zxcvbn ≥ 3 (US-01).
pub fn check_master_password(password: &SecretString) -> Result<(), VaultError> {
    let score = u8::from(zxcvbn::zxcvbn(password.expose_secret(), &[]).score());
    if score < MIN_SCORE {
        return Err(VaultError::WeakPassword(score));
    }
    Ok(())
}
