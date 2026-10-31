use secrecy::{ExposeSecret, SecretString};

use super::VaultError;

pub const MIN_SCORE: u8 = 3;

/// Score zxcvbn (0-4). Au-delà de 100 caractères on tronque : zxcvbn est coûteux sur les
/// longues chaînes et le score y est de toute façon maximal.
pub fn password_score(password: &SecretString) -> u8 {
    let s: String = password.expose_secret().chars().take(100).collect();
    u8::from(zxcvbn::zxcvbn(&s, &[]).score())
}

/// Exige un mot de passe maître d'un score zxcvbn ≥ 3 (US-01).
pub fn check_master_password(password: &SecretString) -> Result<(), VaultError> {
    let score = password_score(password);
    if score < MIN_SCORE {
        return Err(VaultError::WeakPassword(score));
    }
    Ok(())
}
