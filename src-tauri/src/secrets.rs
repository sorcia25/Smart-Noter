//! Windows DPAPI wrapper: encrypt/decrypt small secrets (API keys) bound to the
//! current Windows user profile. Ciphertext is opaque and only decryptable by the
//! same user on the same machine. Plaintext never persists.

use windows::Win32::Foundation::{LocalFree, HLOCAL};
use windows::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB,
};

/// Encrypt `plaintext` with DPAPI. Returns opaque ciphertext bytes.
pub fn encrypt(plaintext: &str) -> Result<Vec<u8>, String> {
    let mut bytes = plaintext.as_bytes().to_vec();
    let in_blob = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_mut_ptr(),
    };
    let mut out_blob = CRYPT_INTEGER_BLOB::default();
    unsafe {
        CryptProtectData(&in_blob, None, None, None, None, 0, &mut out_blob)
            .map_err(|e| format!("DPAPI encrypt failed: {e}"))?;
        let slice = std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize);
        let result = slice.to_vec();
        let _ = LocalFree(Some(HLOCAL(out_blob.pbData.cast())));
        Ok(result)
    }
}

/// Decrypt DPAPI `ciphertext` back to the original UTF-8 string.
pub fn decrypt(ciphertext: &[u8]) -> Result<String, String> {
    let mut data = ciphertext.to_vec();
    let in_blob = CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_mut_ptr(),
    };
    let mut out_blob = CRYPT_INTEGER_BLOB::default();
    unsafe {
        CryptUnprotectData(&in_blob, None, None, None, None, 0, &mut out_blob)
            .map_err(|e| format!("DPAPI decrypt failed: {e}"))?;
        let slice = std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize);
        let result = String::from_utf8(slice.to_vec()).map_err(|e| e.to_string());
        let _ = LocalFree(Some(HLOCAL(out_blob.pbData.cast())));
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_round_trip() {
        let secret = "sk-test-ABC123-áé"; // includes multibyte to prove UTF-8 safety
        let ct = encrypt(secret).unwrap();
        assert_ne!(
            ct.as_slice(),
            secret.as_bytes(),
            "ciphertext must differ from plaintext"
        );
        assert_eq!(decrypt(&ct).unwrap(), secret);
    }

    #[test]
    fn decrypt_garbage_errors() {
        assert!(decrypt(&[0u8, 1, 2, 3, 4]).is_err());
    }
}
