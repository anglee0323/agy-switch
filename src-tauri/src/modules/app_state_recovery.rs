//! Recover only the observed all-NUL App state, retaining the original bytes.
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use std::path::{Path, PathBuf};
use windows_sys::Win32::Foundation::GENERIC_READ;
use windows_sys::Win32::Storage::FileSystem::{
    FileRenameInfo, SetFileInformationByHandle, DELETE, FILE_FLAG_OPEN_REPARSE_POINT,
    FILE_RENAME_INFO,
};

const MAX_STATE_BYTES: u64 = 16 * 1024 * 1024;
const FILE_SHARE_DELETE: u32 = 0x0000_0004;

pub(crate) fn recover_default() -> Result<(), String> {
    let home = dirs::home_dir().ok_or("Cannot locate the Antigravity state directory")?;
    if let Some(backup) = recover(&home, crate::modules::process::state_writer_running)? {
        crate::modules::logger::log_warn(&format!(
            "Backed up all-zero Antigravity App state to {}",
            backup.display()
        ));
    }
    Ok(())
}

fn all_zero(file: &mut File) -> io::Result<bool> {
    let mut buf = [0; 8192];
    loop {
        let count = file.read(&mut buf)?;
        if count == 0 {
            return Ok(true);
        }
        if buf[..count].iter().any(|byte| *byte != 0) {
            return Ok(false);
        }
    }
}

fn rename_open_file(file: &File, destination: &Path) -> io::Result<()> {
    let name: Vec<u16> = destination.as_os_str().encode_wide().collect();
    let name_bytes = name
        .len()
        .checked_mul(2)
        .ok_or_else(|| io::Error::other("Backup path too long"))?;
    let size = std::mem::offset_of!(FILE_RENAME_INFO, FileName)
        .checked_add(name_bytes)
        .ok_or_else(|| io::Error::other("Backup path too long"))?;
    let size_u32 = u32::try_from(size).map_err(|_| io::Error::other("Backup path too long"))?;
    let mut storage = vec![0usize; size.div_ceil(std::mem::size_of::<usize>())];
    let info = storage.as_mut_ptr().cast::<FILE_RENAME_INFO>();
    unsafe {
        (*info).Anonymous.ReplaceIfExists = false;
        (*info).RootDirectory = std::ptr::null_mut();
        (*info).FileNameLength = name_bytes as u32;
        std::ptr::copy_nonoverlapping(name.as_ptr(), (*info).FileName.as_mut_ptr(), name.len());
        if SetFileInformationByHandle(file.as_raw_handle(), FileRenameInfo, info.cast(), size_u32)
            == 0
        {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

pub(crate) fn recover(
    home: &Path,
    writer_running: impl FnOnce() -> bool,
) -> Result<Option<PathBuf>, String> {
    let state = home.join(".gemini/antigravity/antigravity_state.pbtxt");
    let meta = match fs::symlink_metadata(&state) {
        Ok(meta) => meta,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("Cannot inspect Antigravity state: {e}")),
    };
    if !meta.file_type().is_file() || meta.len() == 0 || meta.len() > MAX_STATE_BYTES {
        return Ok(None);
    }
    let mut probe =
        File::open(&state).map_err(|e| format!("Cannot inspect Antigravity state: {e}"))?;
    if !all_zero(&mut probe).map_err(|e| format!("Cannot inspect Antigravity state: {e}"))? {
        return Ok(None);
    }
    drop(probe);
    // Hold a handle that denies concurrent reads and writes. Rename that exact
    // handle, never an unchecked replacement that might appear at the same path.
    let mut locked = OpenOptions::new()
        .read(true)
        .access_mode(GENERIC_READ | DELETE)
        .share_mode(FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(&state)
        .map_err(|e| {
            format!("Cannot lock corrupt Antigravity state; close Antigravity and retry: {e}")
        })?;
    let locked_meta = locked
        .metadata()
        .map_err(|e| format!("Cannot inspect Antigravity state: {e}"))?;
    let len = locked_meta.len();
    if !locked_meta.file_type().is_file()
        || len == 0
        || len > MAX_STATE_BYTES
        || !all_zero(&mut locked).map_err(|e| format!("Cannot inspect Antigravity state: {e}"))?
    {
        return Ok(None);
    }
    if writer_running() {
        return Err("Corrupt Antigravity state found. Fully close Antigravity, agy and language servers, then retry; credentials were not changed".into());
    }
    let backup = state.with_file_name(format!(
        "antigravity_state.pbtxt.corrupt-{}",
        uuid::Uuid::new_v4()
    ));
    rename_open_file(&locked, &backup)
        .map_err(|e| format!("Cannot back up corrupt Antigravity state: {e}"))?;
    drop(locked);
    Ok(Some(backup))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let state = dir
            .path()
            .join(".gemini/antigravity/antigravity_state.pbtxt");
        fs::create_dir_all(state.parent().unwrap()).unwrap();
        (dir, state)
    }

    #[test]
    fn recovers_all_zero_state_and_preserves_original_bytes() {
        let (dir, state) = fixture();
        let original = vec![0; 1195];
        fs::write(&state, &original).unwrap();
        let backup = recover(dir.path(), || false).unwrap().unwrap();
        assert!(!state.exists());
        assert_eq!(fs::read(backup).unwrap(), original);
        assert!(recover(dir.path(), || false).unwrap().is_none());
    }

    #[test]
    fn leaves_other_inputs_untouched_and_blocks_active_writers() {
        let (dir, state) = fixture();
        for bytes in [
            b"".as_slice(),
            b"state: 1\n".as_slice(),
            b"\0x\0".as_slice(),
        ] {
            fs::write(&state, bytes).unwrap();
            assert!(recover(dir.path(), || panic!("not corrupt"))
                .unwrap()
                .is_none());
            assert_eq!(fs::read(&state).unwrap(), bytes);
        }
        fs::write(&state, vec![0; 1195]).unwrap();
        assert!(recover(dir.path(), || true).is_err());
        assert_eq!(fs::read(&state).unwrap(), vec![0; 1195]);
        assert_eq!(fs::read_dir(state.parent().unwrap()).unwrap().count(), 1);
    }

    #[test]
    fn locked_or_oversized_state_is_never_moved() {
        let (dir, state) = fixture();
        fs::write(&state, vec![0; 1195]).unwrap();
        let locked = OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(0)
            .open(&state)
            .unwrap();
        assert!(recover(dir.path(), || false).is_err());
        drop(locked);
        assert_eq!(fs::read(&state).unwrap(), vec![0; 1195]);
        let file = OpenOptions::new().write(true).open(&state).unwrap();
        file.set_len(MAX_STATE_BYTES + 1).unwrap();
        drop(file);
        assert!(recover(dir.path(), || panic!("oversized"))
            .unwrap()
            .is_none());
        assert_eq!(fs::metadata(state).unwrap().len(), MAX_STATE_BYTES + 1);
    }

    #[test]
    fn backup_rename_never_overwrites_existing_file() {
        let (dir, state) = fixture();
        fs::write(&state, [0; 3]).unwrap();
        let destination = dir.path().join("existing-backup");
        fs::write(&destination, b"keep").unwrap();
        let file = OpenOptions::new()
            .read(true)
            .access_mode(GENERIC_READ | DELETE)
            .share_mode(FILE_SHARE_DELETE)
            .open(&state)
            .unwrap();
        assert!(rename_open_file(&file, &destination).is_err());
        drop(file);
        assert_eq!(fs::read(&destination).unwrap(), b"keep");
        assert_eq!(fs::read(&state).unwrap(), [0; 3]);
    }
}
