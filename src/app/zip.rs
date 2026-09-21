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
    let mut entries = Vec::with_capacity(files.len());
    let mut names = Vec::with_capacity(files.len());

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
        names.push(name.clone());
        entries.push((name, data));
    }
    write_zip_entries(dest, &entries)?;
    Ok(names)
}

/// Write named in-memory byte entries as a STORED zip (E213 — profile
/// export stages JSON without temp files).
pub fn write_zip_entries(dest: &Path, entries: &[(String, Vec<u8>)]) -> Result<(), String> {
    let mut out = std::fs::File::create(dest).map_err(|e| e.to_string())?;
    let mut central = Vec::new();
    let (dos_time, dos_date) = dos_datetime();

    for (name, data) in entries {
        let name = name.clone();
        let data = data.as_slice();
        let crc = crc32(data);
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
        out.write_all(data).map_err(ioe)?;
        central.push((name, crc, sz, offset));
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
    Ok(())
}

fn central_offset(f: &std::fs::File) -> Result<u32, String> {
    f.metadata()
        .map(|m| m.len() as u32)
        .map_err(|e| e.to_string())
}

fn ioe(e: std::io::Error) -> String {
    e.to_string()
}

// ── Reader (E213 profile import) — stored entries only, CRC-verified ──────────

fn u16_at(d: &[u8], off: usize) -> Result<u16, String> {
    d.get(off..off + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .ok_or_else(|| "truncated zip".to_string())
}

fn u32_at(d: &[u8], off: usize) -> Result<u32, String> {
    d.get(off..off + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or_else(|| "truncated zip".to_string())
}

/// Read a store-only zip — the format `write_zip` emits — into
/// `(name, bytes)` pairs. Compressed entries error honestly (we never
/// inflate); CRC is verified per entry.
pub fn read_zip_stored(path: &Path) -> Result<Vec<(String, Vec<u8>)>, String> {
    let data = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    // EOCD lives in the last 64 KiB + 22 bytes.
    let tail_start = data.len().saturating_sub(66_000);
    let mut cd = None;
    for i in (tail_start..data.len().saturating_sub(3)).rev() {
        if data.get(i..i + 4) == Some(b"PK\x05\x06") {
            let count = u16_at(&data, i + 10)? as usize;
            let off = u32_at(&data, i + 16)? as usize;
            cd = Some((count, off));
            break;
        }
    }
    let (count, mut p) = cd.ok_or("no end-of-central-directory — not a zip?")?;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        if data.get(p..p + 4) != Some(b"PK\x01\x02") {
            return Err("bad central-directory record".into());
        }
        if u16_at(&data, p + 10)? != 0 {
            return Err("zip entry is compressed — only stored zips are supported".into());
        }
        let crc = u32_at(&data, p + 16)?;
        let comp = u32_at(&data, p + 20)? as usize;
        let name_len = u16_at(&data, p + 28)? as usize;
        let extra_len = u16_at(&data, p + 30)? as usize;
        let comment_len = u16_at(&data, p + 32)? as usize;
        let lh_off = u32_at(&data, p + 42)? as usize;
        let name = data
            .get(p + 46..p + 46 + name_len)
            .map(|b| String::from_utf8_lossy(b).to_string())
            .ok_or("truncated zip")?;
        if data.get(lh_off..lh_off + 4) != Some(b"PK\x03\x04") {
            return Err("bad local header".into());
        }
        let lnl = u16_at(&data, lh_off + 26)? as usize;
        let lel = u16_at(&data, lh_off + 28)? as usize;
        let dstart = lh_off + 30 + lnl + lel;
        let bytes = data
            .get(dstart..dstart + comp)
            .ok_or("truncated zip entry")?
            .to_vec();
        if crc32(&bytes) != crc {
            return Err(format!("crc mismatch on {name}"));
        }
        out.push((name, bytes));
        p += 46 + name_len + extra_len + comment_len;
    }
    Ok(out)
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

    #[test]
    fn entries_write_then_read_roundtrip() {
        let dir = std::env::temp_dir().join(format!("vibecap_ziprd_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let zip = dir.join("p.zip");
        write_zip_entries(
            &zip,
            &[
                ("manifest.json".into(), br#"{"kind":"x"}"#.to_vec()),
                ("session.json".into(), b"{\"theme\":\"dark\"}".to_vec()),
            ],
        )
        .unwrap();
        let got = read_zip_stored(&zip).unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].0, "manifest.json");
        assert_eq!(got[1].0, "session.json");
        assert_eq!(got[1].1, b"{\"theme\":\"dark\"}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_zip_rejects_garbage() {
        let dir = std::env::temp_dir().join(format!("vibecap_zipbad_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let zip = dir.join("not.zip");
        std::fs::write(&zip, b"definitely not a zip file").unwrap();
        assert!(read_zip_stored(&zip).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
