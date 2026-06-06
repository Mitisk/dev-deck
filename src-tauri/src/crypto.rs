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
