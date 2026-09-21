//! Store-only ZIP writer — no compression, no dep.
//!
//! Enough for "export selection as ZIP": entries are STORED (method 0),
//! which every unzipper accepts. Filenames are sanitized to a basename and
//! deduped so two same-named captures in different dirs don't collide.

use std::io::Write;
use std::path::{Path, PathBuf};

/// CRC-32 (IEEE) — the polynomial ZIP uses.
fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (i, e) in table.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
        *e = c;
    }
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc = table[((crc ^ b as u32) & 0xFF) as usize] ^ (crc >> 8);
    }
    crc ^ 0xFFFF_FFFF
}

/// MS-DOS date/time packed into the ZIP header (from `chrono` — already a dep).
fn dos_datetime() -> (u16, u16) {
    let now = chrono::Local::now();
    use chrono::Datelike;
    let (y, m, d) = (
        now.year().clamp(1980, 2107) as u16,
        now.month() as u16,
        now.day() as u16,
    );
    use chrono::Timelike;
    let (hh, mm, ss) = (now.hour() as u16, now.minute() as u16, now.second() as u16);
    let dos_time = (hh << 11) | (mm << 5) | (ss / 2);
    let dos_date = ((y - 1980) << 9) | (m << 5) | d;
    (dos_time, dos_date)
}

/// Basename + zip-safe characters only; empty → "file".
fn entry_name(path: &Path) -> String {
    let raw = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".into());
    let clean: String = raw
        .chars()
        .map(|c| match c {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect();
    if clean.trim().is_empty() {
        "file".into()
    } else {
        clean
    }
}

/// Write `files` as a STORED zip at `dest`. Returns the entry names used
/// (deduped with `_2`, `_3`, … suffixes on collisions).
pub fn write_zip(dest: &Path, files: &[PathBuf]) -> Result<Vec<String>, String> {
    let mut used = std::collections::HashSet::new();
    let mut out = std::fs::File::create(dest).map_err(|e| e.to_string())?;
    let mut central = Vec::new();
    let mut names = Vec::new();
    let (dos_time, dos_date) = dos_datetime();

    for path in files {
        let data = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let base = entry_name(path);
        let mut name = base.clone();
        let mut n = 2u32;
        while !used.insert(name.clone()) {
            let stem = base.rsplit_once('.').map(|(s, _)| s).unwrap_or(&base);
            let ext = base.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
            name = if ext.is_empty() {
                format!("{stem}_{n}")
            } else {
                format!("{stem}_{n}.{ext}")
            };
            n += 1;
        }
        let crc = crc32(&data);
        let offset = central_offset(&out)?;
        // Local file header
        out.write_all(&0x0403_4b50u32.to_le_bytes()).map_err(ioe)?;
        out.write_all(&20u16.to_le_bytes()).map_err(ioe)?; // version needed
        out.write_all(&0u16.to_le_bytes()).map_err(ioe)?; // flags
        out.write_all(&0u16.to_le_bytes()).map_err(ioe)?; // method = store
        out.write_all(&dos_time.to_le_bytes()).map_err(ioe)?;
        out.write_all(&dos_date.to_le_bytes()).map_err(ioe)?;
        out.write_all(&crc.to_le_bytes()).map_err(ioe)?;
        let sz = data.len() as u32;
        out.write_all(&sz.to_le_bytes()).map_err(ioe)?;
        out.write_all(&sz.to_le_bytes()).map_err(ioe)?;
        out.write_all(&(name.len() as u16).to_le_bytes())
            .map_err(ioe)?;
        out.write_all(&0u16.to_le_bytes()).map_err(ioe)?; // extra len
        out.write_all(name.as_bytes()).map_err(ioe)?;
        out.write_all(&data).map_err(ioe)?;
        central.push((name.clone(), crc, sz, offset));
        names.push(name);
    }

    let cd_start = central_offset(&out)?;
    for (name, crc, sz, offset) in &central {
        out.write_all(&0x0201_4b50u32.to_le_bytes()).map_err(ioe)?;
        out.write_all(&20u16.to_le_bytes()).map_err(ioe)?; // version made by
        out.write_all(&20u16.to_le_bytes()).map_err(ioe)?; // version needed
        out.write_all(&0u16.to_le_bytes()).map_err(ioe)?; // flags
        out.write_all(&0u16.to_le_bytes()).map_err(ioe)?; // method
        out.write_all(&dos_time.to_le_bytes()).map_err(ioe)?;
        out.write_all(&dos_date.to_le_bytes()).map_err(ioe)?;
        out.write_all(&crc.to_le_bytes()).map_err(ioe)?;
        out.write_all(&sz.to_le_bytes()).map_err(ioe)?;
        out.write_all(&sz.to_le_bytes()).map_err(ioe)?;
        out.write_all(&(name.len() as u16).to_le_bytes())
            .map_err(ioe)?;
        out.write_all(&0u16.to_le_bytes()).map_err(ioe)?; // extra
        out.write_all(&0u16.to_le_bytes()).map_err(ioe)?; // comment
        out.write_all(&0u16.to_le_bytes()).map_err(ioe)?; // disk
        out.write_all(&0u16.to_le_bytes()).map_err(ioe)?; // int attrs
        out.write_all(&0u32.to_le_bytes()).map_err(ioe)?; // ext attrs
        out.write_all(&offset.to_le_bytes()).map_err(ioe)?;
        out.write_all(name.as_bytes()).map_err(ioe)?;
    }
    let cd_size = central_offset(&out)? - cd_start;
    out.write_all(&0x0605_4b50u32.to_le_bytes()).map_err(ioe)?; // EOCD
    out.write_all(&0u16.to_le_bytes()).map_err(ioe)?; // disk
    out.write_all(&0u16.to_le_bytes()).map_err(ioe)?; // cd disk
    let count = central.len() as u16;
    out.write_all(&count.to_le_bytes()).map_err(ioe)?;
    out.write_all(&count.to_le_bytes()).map_err(ioe)?;
    out.write_all(&cd_size.to_le_bytes()).map_err(ioe)?;
    out.write_all(&cd_start.to_le_bytes()).map_err(ioe)?;
    out.write_all(&0u16.to_le_bytes()).map_err(ioe)?; // comment len
    Ok(names)
}

fn central_offset(f: &std::fs::File) -> Result<u32, String> {
    f.metadata()
        .map(|m| m.len() as u32)
        .map_err(|e| e.to_string())
}

fn ioe(e: std::io::Error) -> String {
    e.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_known_vectors() {
        assert_eq!(crc32(b""), 0);
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn write_zip_roundtrip_structure() {
        let dir = std::env::temp_dir().join(format!("vibecap_ziptest_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a.txt");
        let b = dir.join("b.txt");
        std::fs::write(&a, b"hello").unwrap();
        std::fs::write(&b, b"world!").unwrap();
        let zip = dir.join("out.zip");
        let names = write_zip(&zip, &[a.clone(), b.clone()]).unwrap();
        assert_eq!(names, vec!["a.txt", "b.txt"]);

        let bytes = std::fs::read(&zip).unwrap();
        assert_eq!(&bytes[..4], &0x0403_4b50u32.to_le_bytes());
        // EOCD magic near the end.
        assert!(bytes.windows(4).any(|w| w == 0x0605_4b50u32.to_le_bytes()));
        // Both payloads present verbatim (stored, not compressed).
        assert!(bytes.windows(5).any(|w| w == b"hello"));
        assert!(bytes.windows(6).any(|w| w == b"world!"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_zip_dedupes_names() {
        let dir = std::env::temp_dir().join(format!("vibecap_zipdup_{}", std::process::id()));
        let d1 = dir.join("one");
        let d2 = dir.join("two");
        std::fs::create_dir_all(&d1).unwrap();
        std::fs::create_dir_all(&d2).unwrap();
        let f1 = d1.join("same.png");
        let f2 = d2.join("same.png");
        std::fs::write(&f1, b"1").unwrap();
        std::fs::write(&f2, b"2").unwrap();
        let zip = dir.join("out.zip");
        let names = write_zip(&zip, &[f1, f2]).unwrap();
        assert_eq!(names, vec!["same.png", "same_2.png"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
