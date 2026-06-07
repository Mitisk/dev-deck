use crate::error::{AppError, AppResult};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{LocalFree, HLOCAL};
use windows::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB,
};

/// Зашифровать секрет (DPAPI, привязка к учётке Windows).
///
/// В версии `windows` 0.61 `CryptProtectData` и `CryptUnprotectData` имеют
/// РАЗНЫЕ сигнатуры второго аргумента (`Param<PCWSTR>` против `Option<*mut PWSTR>`),
/// поэтому единая обёртка через указатель на функцию не типизируется — два прямых
/// `unsafe`-вызова.
pub fn encrypt(plain: &[u8]) -> AppResult<Vec<u8>> {
    unsafe {
        let input = CRYPT_INTEGER_BLOB {
            cbData: plain.len() as u32,
            pbData: plain.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB::default();
        CryptProtectData(&input, PCWSTR::null(), None, None, None, 0, &mut output)
            .map_err(|e| AppError::internal(format!("DPAPI encrypt: {}", e)))?;
        let out = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        let _ = LocalFree(Some(HLOCAL(output.pbData as *mut core::ffi::c_void)));
        Ok(out)
    }
}

/// Расшифровать секрет.
pub fn decrypt(cipher: &[u8]) -> AppResult<Vec<u8>> {
    unsafe {
        let input = CRYPT_INTEGER_BLOB {
            cbData: cipher.len() as u32,
            pbData: cipher.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB::default();
        CryptUnprotectData(&input, None, None, None, None, 0, &mut output)
            .map_err(|e| AppError::internal(format!("DPAPI decrypt: {}", e)))?;
        let out = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        let _ = LocalFree(Some(HLOCAL(output.pbData as *mut core::ffi::c_void)));
        Ok(out)
    }
}

// ---------- AES-256-GCM + Argon2id (режим мастер-пароля) ----------

use aes_gcm::aead::{Aead, AeadCore, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use argon2::Argon2;
use rand::rngs::OsRng;

/// 16 случайных байт соли.
pub fn random_salt() -> [u8; 16] {
    use rand::RngCore;
    let mut s = [0u8; 16];
    OsRng.fill_bytes(&mut s);
    s
}

/// Argon2id: пароль + соль → 32-байтный ключ.
pub fn derive_key(password: &str, salt: &[u8]) -> AppResult<[u8; 32]> {
    let mut key = [0u8; 32];
    Argon2::default()
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|e| AppError::internal(format!("argon2: {}", e)))?;
    Ok(key)
}

/// AES-256-GCM: вернуть nonce(12) || ciphertext.
pub fn aes_encrypt(key: &[u8; 32], plain: &[u8]) -> AppResult<Vec<u8>> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ct = cipher
        .encrypt(&nonce, plain)
        .map_err(|_| AppError::internal("aes encrypt"))?;
    let mut out = nonce.to_vec();
    out.extend_from_slice(&ct);
    Ok(out)
}

/// Расшифровать nonce(12) || ciphertext.
pub fn aes_decrypt(key: &[u8; 32], blob: &[u8]) -> AppResult<Vec<u8>> {
    if blob.len() < 12 {
        return Err(AppError::internal("aes blob too short"));
    }
    let (n, ct) = blob.split_at(12);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    cipher
        .decrypt(Nonce::from_slice(n), ct)
        .map_err(|_| AppError::internal("aes decrypt (неверный ключ?)"))
}

#[cfg(test)]
mod aes_tests {
    use super::*;

    #[test]
    fn aes_roundtrip() {
        let key = derive_key("hunter2", b"saltsaltsaltsalt").unwrap();
        let enc = aes_encrypt(&key, b"secret-value").unwrap();
        assert_ne!(&enc[12..], b"secret-value");
        assert_eq!(aes_decrypt(&key, &enc).unwrap(), b"secret-value");
    }

    #[test]
    fn wrong_key_fails() {
        let salt = b"saltsaltsaltsalt";
        let k1 = derive_key("a", salt).unwrap();
        let k2 = derive_key("b", salt).unwrap();
        let enc = aes_encrypt(&k1, b"x").unwrap();
        assert!(aes_decrypt(&k2, &enc).is_err());
    }

    #[test]
    fn derive_is_deterministic() {
        let salt = super::random_salt();
        assert_eq!(derive_key("p", &salt).unwrap(), derive_key("p", &salt).unwrap());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let secret = b"sk_test_51Hx9pQ2eZvKYlo2C";
        let enc = encrypt(secret).unwrap();
        assert_ne!(
            enc.as_slice(),
            secret.as_slice(),
            "ciphertext must differ from plaintext"
        );
        let dec = decrypt(&enc).unwrap();
        assert_eq!(dec.as_slice(), secret.as_slice());
    }
}
