//! Current-user DPAPI protection for host authentication records.
use kokoda_media::BackendResult as Result;
use std::{io::Read, path::Path, ptr::null_mut};
use winapi::um::{
    dpapi::{CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN},
    winbase::LocalFree,
    wincrypt::DATA_BLOB,
};
use zeroize::{Zeroize, Zeroizing};
fn transform(bytes: &[u8], encrypt: bool) -> Result<Zeroizing<Vec<u8>>> {
    if bytes.len() > 65536 {
        return Err("Credential record too large".into());
    }
    let mut input = DATA_BLOB {
        cbData: bytes.len() as _,
        pbData: bytes.as_ptr() as _,
    };
    let mut output = DATA_BLOB {
        cbData: 0,
        pbData: null_mut(),
    };
    let success = unsafe {
        if encrypt {
            CryptProtectData(
                &mut input,
                std::ptr::null(),
                null_mut(),
                null_mut(),
                null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &mut input,
                null_mut(),
                null_mut(),
                null_mut(),
                null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        }
    };
    if success == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    unsafe {
        let data = std::slice::from_raw_parts_mut(output.pbData, output.cbData as usize);
        let result = Zeroizing::new(data.to_vec());
        data.zeroize();
        LocalFree(output.pbData as _);
        Ok(result)
    }
}
pub fn save(path: &Path, record: &[u8]) -> Result<()> {
    let sealed = transform(record, true)?;
    let parent = path.parent().ok_or("Missing credential parent directory")?;
    std::fs::create_dir_all(parent)?;
    let temp = path.with_extension("tmp");
    std::fs::write(&temp, &sealed)?;
    std::fs::rename(temp, path)?;
    Ok(())
}
pub fn load(path: &Path) -> Result<Zeroizing<Vec<u8>>> {
    let mut bytes = Zeroizing::new(Vec::new());
    std::fs::File::open(path)?
        .take(65537)
        .read_to_end(&mut bytes)?;
    transform(&bytes, false)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn current_user_protection_roundtrip_and_tamper_rejection() {
        let secret = b"synthetic credential record";
        let encrypted = transform(secret, true).unwrap();
        assert!(!encrypted.windows(secret.len()).any(|w| w == secret));
        assert_eq!(transform(&encrypted, false).unwrap().as_slice(), secret);
        let mut changed = encrypted.to_vec();
        let index = changed.len() / 2;
        changed[index] ^= 1;
        assert!(transform(&changed, false).is_err());
    }
}
