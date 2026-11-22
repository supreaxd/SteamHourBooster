use base64::Engine;
use base64::prelude::BASE64_STANDARD;

#[repr(C)]
struct DataBlob {
    cb_data: u32,
    pb_data: *mut u8,
}

#[link(name = "crypt32")]
extern "system" {
    fn CryptProtectData(
        p_data_in: *const DataBlob,
        sz_data_descr: *const u16,
        p_optional_entropy: *const DataBlob,
        pv_reserved: *mut std::ffi::c_void,
        p_prompt_struct: *mut std::ffi::c_void,
        dw_flags: u32,
        p_data_out: *mut DataBlob,
    ) -> i32;

    fn CryptUnprotectData(
        p_data_in: *const DataBlob,
        ppsz_data_descr: *mut *mut u16,
        p_optional_entropy: *const DataBlob,
        pv_reserved: *mut std::ffi::c_void,
        p_prompt_struct: *mut std::ffi::c_void,
        dw_flags: u32,
        p_data_out: *mut DataBlob,
    ) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn LocalFree(h_mem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
}

pub fn encrypt_string(plaintext: &str) -> String {
    if plaintext.trim().is_empty() {
        return plaintext.to_string();
    }
    let mut bytes = plaintext.as_bytes().to_vec();
    let in_blob = DataBlob {
        cb_data: bytes.len() as u32,
        pb_data: bytes.as_mut_ptr(),
    };
    let mut out_blob = DataBlob {
        cb_data: 0,
        pb_data: std::ptr::null_mut(),
    };
    let success = unsafe {
        CryptProtectData(
            &in_blob,
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0x01, // CRYPTPROTECT_UI_FORBIDDEN
            &mut out_blob,
        )
    };
    if success != 0 && !out_blob.pb_data.is_null() {
        let slice = unsafe { std::slice::from_raw_parts(out_blob.pb_data, out_blob.cb_data as usize) };
        let b64 = BASE64_STANDARD.encode(slice);
        unsafe { LocalFree(out_blob.pb_data as *mut _) };
        format!("enc:{}", b64)
    } else {
        plaintext.to_string()
    }
}

pub fn decrypt_string(ciphertext: &str) -> String {
    if !ciphertext.starts_with("enc:") {
        return ciphertext.to_string();
    }
    let b64 = &ciphertext[4..];
    let decoded = match BASE64_STANDARD.decode(b64) {
        Ok(d) => d,
        Err(_) => return ciphertext.to_string(),
    };
    let mut bytes = decoded;
    let in_blob = DataBlob {
        cb_data: bytes.len() as u32,
        pb_data: bytes.as_mut_ptr(),
    };
    let mut out_blob = DataBlob {
        cb_data: 0,
        pb_data: std::ptr::null_mut(),
    };
    let success = unsafe {
        CryptUnprotectData(
            &in_blob,
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0x01, // CRYPTPROTECT_UI_FORBIDDEN
            &mut out_blob,
        )
    };
    if success != 0 && !out_blob.pb_data.is_null() {
        let slice = unsafe { std::slice::from_raw_parts(out_blob.pb_data, out_blob.cb_data as usize) };
        let res = String::from_utf8_lossy(slice).to_string();
        unsafe { LocalFree(out_blob.pb_data as *mut _) };
        res
    } else {
        ciphertext.to_string()
    }
}
