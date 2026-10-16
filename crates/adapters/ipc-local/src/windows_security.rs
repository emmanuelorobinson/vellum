//! The only Windows unsafe boundary: owned OS allocations and synchronous security API calls.
#![allow(unsafe_code)]
use std::{
    ffi::c_void,
    fs, io,
    os::windows::{
        ffi::OsStrExt,
        fs::MetadataExt,
        io::{FromRawHandle, OwnedHandle},
    },
    path::Path,
    ptr,
};
use windows_sys::Win32::{
    Foundation::LocalFree,
    Security::{self as sec, Authorization as auth},
    Storage::FileSystem::{CreateDirectoryW, FILE_ATTRIBUTE_REPARSE_POINT},
    System::Threading::{GetCurrentProcess, OpenProcessToken},
};

struct Local(*mut c_void);
impl Drop for Local {
    fn drop(&mut self) {
        unsafe {
            LocalFree(self.0);
        }
    }
}
fn checked(ok: i32) -> io::Result<()> {
    if ok == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
fn wide(value: &std::ffi::OsStr) -> Vec<u16> {
    value.encode_wide().chain(Some(0)).collect()
}
fn denied() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "runtime path must have a current-user-only owner and DACL",
    )
}

pub(crate) fn sid() -> io::Result<String> {
    // All buffers outlive API calls; TOKEN_USER buffer uses pointer alignment. Handles/LocalAlloc
    // results are released by RAII, including every early-return path.
    unsafe {
        let mut raw = ptr::null_mut();
        checked(OpenProcessToken(
            GetCurrentProcess(),
            sec::TOKEN_QUERY,
            &mut raw,
        ))?;
        let _handle = OwnedHandle::from_raw_handle(raw);
        let mut length = 0;
        sec::GetTokenInformation(raw, sec::TokenUser, ptr::null_mut(), 0, &mut length);
        if length == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut buffer = vec![0_usize; (length as usize).div_ceil(size_of::<usize>())];
        checked(sec::GetTokenInformation(
            raw,
            sec::TokenUser,
            buffer.as_mut_ptr().cast(),
            length,
            &mut length,
        ))?;
        let user = &*buffer.as_ptr().cast::<sec::TOKEN_USER>();
        sid_string(user.User.Sid)
    }
}
unsafe fn sid_string(sid: sec::PSID) -> io::Result<String> {
    unsafe {
        let mut text = ptr::null_mut();
        checked(auth::ConvertSidToStringSidW(sid, &mut text))?;
        let _allocation = Local(text.cast());
        let mut len = 0;
        while *text.add(len) != 0 {
            len += 1;
        }
        String::from_utf16(std::slice::from_raw_parts(text, len)).map_err(|_| denied())
    }
}
fn descriptor(directory: bool) -> io::Result<Local> {
    let user = sid()?;
    let inherit = if directory { "OICI" } else { "" };
    let text = wide(std::ffi::OsStr::new(&format!(
        "O:{user}D:P(A;{inherit};FA;;;{user})"
    )));
    let mut descriptor = ptr::null_mut();
    unsafe {
        checked(auth::ConvertStringSecurityDescriptorToSecurityDescriptorW(
            text.as_ptr(),
            1,
            &mut descriptor,
            ptr::null_mut(),
        ))?;
    }
    Ok(Local(descriptor))
}
fn attributes(descriptor: &Local) -> sec::SECURITY_ATTRIBUTES {
    sec::SECURITY_ATTRIBUTES {
        nLength: size_of::<sec::SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: 0,
    }
}
pub(super) fn prepare_dir(path: &Path) -> io::Result<()> {
    let descriptor = descriptor(true)?;
    let attrs = attributes(&descriptor);
    let path_wide = wide(path.as_os_str());
    let result = unsafe { CreateDirectoryW(path_wide.as_ptr(), &attrs) };
    if result == 0 && io::Error::last_os_error().kind() != io::ErrorKind::AlreadyExists {
        return Err(io::Error::last_os_error());
    }
    check(path, true)
}
pub(super) fn check_file(path: &Path) -> io::Result<()> {
    check(path, false)
}
fn check(path: &Path, directory: bool) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || metadata.is_dir() != directory
        || (!directory && !metadata.is_file())
    {
        return Err(denied());
    }
    let path = wide(path.as_os_str());
    unsafe {
        let mut owner = ptr::null_mut();
        let mut dacl = ptr::null_mut();
        let mut descriptor = ptr::null_mut();
        let error = auth::GetNamedSecurityInfoW(
            path.as_ptr(),
            auth::SE_FILE_OBJECT,
            sec::OWNER_SECURITY_INFORMATION | sec::DACL_SECURITY_INFORMATION,
            &mut owner,
            ptr::null_mut(),
            &mut dacl,
            ptr::null_mut(),
            &mut descriptor,
        );
        if error != 0 {
            return Err(io::Error::from_raw_os_error(error as i32));
        }
        let _allocation = Local(descriptor);
        if owner.is_null()
            || dacl.is_null()
            || sid_string(owner)? != sid()?
            || (*dacl).AceCount != 1
        {
            return Err(denied());
        }
        let mut ace = ptr::null_mut();
        checked(sec::GetAce(dacl, 0, &mut ace))?;
        let ace = &*ace.cast::<sec::ACCESS_ALLOWED_ACE>();
        // Exactly one allow ACE, granting full file rights to this user, never inherit-only.
        if ace.Header.AceType != 0
            || ace.Header.AceFlags & 8 != 0
            || ace.Mask != 0x001f01ff
            || sid_string((&ace.SidStart as *const u32).cast_mut().cast())? != sid()?
        {
            return Err(denied());
        }
        if directory {
            let mut control = 0;
            let mut revision = 0;
            checked(sec::GetSecurityDescriptorControl(
                descriptor,
                &mut control,
                &mut revision,
            ))?;
            if control & sec::SE_DACL_PROTECTED == 0 {
                return Err(denied());
            }
        }
    }
    Ok(())
}
pub(crate) fn pipe(
    endpoint: &str,
    first: bool,
) -> io::Result<tokio::net::windows::named_pipe::NamedPipeServer> {
    let descriptor = descriptor(false)?;
    let mut attrs = attributes(&descriptor);
    // SAFETY: attrs and the self-relative descriptor remain valid until synchronous CreateNamedPipe
    // returns. The OS copies the descriptor. No remote clients and no inherited process handles.
    unsafe {
        tokio::net::windows::named_pipe::ServerOptions::new()
            .first_pipe_instance(first)
            .reject_remote_clients(true)
            .create_with_security_attributes_raw(
                endpoint,
                (&mut attrs as *mut sec::SECURITY_ATTRIBUTES).cast(),
            )
    }
}

pub(super) fn open_file(
    path: &Path,
    create: bool,
    exclusive: bool,
    write: bool,
) -> io::Result<fs::File> {
    use windows_sys::Win32::{
        Foundation::{GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE},
        Storage::FileSystem::{
            CREATE_NEW, CreateFileW, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_DELETE,
            FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_ALWAYS, OPEN_EXISTING,
        },
    };
    let descriptor = descriptor(false)?;
    let attrs = attributes(&descriptor);
    let disposition = if exclusive {
        CREATE_NEW
    } else if create {
        OPEN_ALWAYS
    } else {
        OPEN_EXISTING
    };
    let name = wide(path.as_os_str());
    // SAFETY: all pointers outlive the call. FromRawHandle takes sole ownership only on success.
    // Explicit owner also covers elevated processes whose default owner would be Administrators.
    let handle = unsafe {
        CreateFileW(
            name.as_ptr(),
            GENERIC_READ | if write { GENERIC_WRITE } else { 0 },
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            &attrs,
            disposition,
            FILE_FLAG_OPEN_REPARSE_POINT,
            ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { fs::File::from_raw_handle(handle) })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn broad_dacl_is_rejected_instead_of_repaired() {
        let path = std::env::temp_dir().join(format!("v-dacl-{}", std::process::id()));
        prepare_dir(&path).unwrap();
        let text = wide(std::ffi::OsStr::new("D:P(A;OICI;FA;;;WD)"));
        let name = wide(path.as_os_str());
        unsafe {
            let mut raw = ptr::null_mut();
            checked(auth::ConvertStringSecurityDescriptorToSecurityDescriptorW(
                text.as_ptr(),
                1,
                &mut raw,
                ptr::null_mut(),
            ))
            .unwrap();
            let broad = Local(raw);
            checked(sec::SetFileSecurityW(
                name.as_ptr(),
                sec::DACL_SECURITY_INFORMATION | sec::PROTECTED_DACL_SECURITY_INFORMATION,
                broad.0,
            ))
            .unwrap();
            assert!(prepare_dir(&path).is_err());
            let private = descriptor(true).unwrap();
            checked(sec::SetFileSecurityW(
                name.as_ptr(),
                sec::DACL_SECURITY_INFORMATION | sec::PROTECTED_DACL_SECURITY_INFORMATION,
                private.0,
            ))
            .unwrap();
        }
        fs::remove_dir(path).unwrap();
    }
}
