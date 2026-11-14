//! Zip archives, just enough for .bw project files: writing (deflated
//! entries) and reading the zips Blockwise writes and the ones a file manager
//! makes (stored or deflated entries; no zip64, no encryption).

use std::io::{Read, Write};

/// The most an archive may unpack to, so a hostile file can't fill the disk.
const MAX_TOTAL: u64 = 512 << 20;

/// A zip holding `entries` (path inside the archive, bytes).
pub fn write(entries: &[(String, Vec<u8>)]) -> std::io::Result<Vec<u8>> {
    let (time, date) = dos_now();
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in entries {
        let mut z = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        z.write_all(data)?;
        let packed = z.finish()?;
        let crc = crc32fast::hash(data);
        let offset = out.len() as u32;
        let fixed = |b: &mut Vec<u8>| {
            u16s(b, &[20, 0x0800, 8, time, date]); // version, UTF-8 names, deflate, time, date
            u32s(b, &[crc, packed.len() as u32, data.len() as u32]);
            u16s(b, &[name.len() as u16, 0]);
        };
        u32s(&mut out, &[0x0403_4b50]);
        fixed(&mut out);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(&packed);

        u32s(&mut central, &[0x0201_4b50]);
        u16s(&mut central, &[20]); // made by
        fixed(&mut central);
        u16s(&mut central, &[0, 0, 0]); // comment, disk, internal attributes
        u32s(&mut central, &[0, offset]); // external attributes, where the entry starts
        central.extend_from_slice(name.as_bytes());
    }
    let (at, n) = (out.len() as u32, entries.len() as u16);
    out.extend_from_slice(&central);
    u32s(&mut out, &[0x0605_4b50]);
    u16s(&mut out, &[0, 0, n, n]);
    u32s(&mut out, &[central.len() as u32, at]);
    u16s(&mut out, &[0]);
    Ok(out)
}

/// Every file in a zip: (path inside the archive, bytes). Folders are left out.
pub fn read(zip: &[u8]) -> Result<Vec<(String, Vec<u8>)>, String> {
    let bad = || "The file is damaged or isn't a zip.".to_string();
    let u16at = |i: usize| zip.get(i..i + 2).map(|b| u16::from_le_bytes([b[0], b[1]]) as usize).ok_or_else(bad);
    let u32at = |i: usize| zip.get(i..i + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize).ok_or_else(bad);
    // the end record is in the last 22 bytes plus at most a 64 KB comment
    let lowest = zip.len().saturating_sub(22 + 0xFFFF);
    let end = (lowest..=zip.len().saturating_sub(22)).rev().find(|&i| zip[i..].starts_with(&[0x50, 0x4b, 0x05, 0x06])).ok_or_else(bad)?;
    let count = u16at(end + 10)?;
    let mut at = u32at(end + 16)?;
    let mut files = Vec::new();
    let mut total = 0u64;
    for _ in 0..count {
        if u32at(at)? != 0x0201_4b50 {
            return Err(bad());
        }
        let (flags, method, crc) = (u16at(at + 8)?, u16at(at + 10)?, u32at(at + 16)? as u32);
        let (packed, size) = (u32at(at + 20)?, u32at(at + 24)?);
        let (nlen, xlen, clen) = (u16at(at + 28)?, u16at(at + 30)?, u16at(at + 32)?);
        let local = u32at(at + 42)?;
        let name = String::from_utf8_lossy(zip.get(at + 46..at + 46 + nlen).ok_or_else(bad)?).replace('\\', "/");
        at += 46 + nlen + xlen + clen;
        if name.ends_with('/') {
            continue;
        }
        if flags & 1 != 0 {
            return Err("The zip is password-protected.".into());
        }
        total += size as u64;
        if total > MAX_TOTAL {
            return Err("The zip is too big to open.".into());
        }
        if u32at(local)? != 0x0403_4b50 {
            return Err(bad());
        }
        let start = local + 30 + u16at(local + 26)? + u16at(local + 28)?;
        let raw = zip.get(start..start + packed).ok_or_else(bad)?;
        let data = match method {
            0 => raw.to_vec(),
            8 => {
                let mut v = Vec::with_capacity(size);
                flate2::read::DeflateDecoder::new(raw).take(size as u64 + 1).read_to_end(&mut v).map_err(|_| bad())?;
                v
            }
            _ => return Err(format!("{name} is packed in a way Blockwise can't unpack.")),
        };
        if data.len() != size || crc32fast::hash(&data) != crc {
            return Err(bad());
        }
        files.push((name, data));
    }
    Ok(files)
}

/// Whether the bytes start like a zip.
pub fn is_zip(b: &[u8]) -> bool {
    b.starts_with(&[0x50, 0x4b, 0x03, 0x04])
}

fn u16s(b: &mut Vec<u8>, v: &[u16]) {
    for x in v {
        b.extend_from_slice(&x.to_le_bytes());
    }
}

fn u32s(b: &mut Vec<u8>, v: &[u32]) {
    for x in v {
        b.extend_from_slice(&x.to_le_bytes());
    }
}

/// Now as a zip (MS-DOS) time and date, in UTC.
fn dos_now() -> (u16, u16) {
    let t = crate::model::now_secs() as i64;
    let (y, m, d) = crate::runtime::civil(t.div_euclid(86400));
    let s = t.rem_euclid(86400);
    let time = (s / 3600) << 11 | ((s / 60) % 60) << 5 | (s % 60) / 2;
    let date = ((y - 1980).clamp(0, 127) as u32) << 9 | m << 5 | d;
    (time as u16, date as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zips_round_trip() {
        let big: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
        let entries = vec![("project.json".to_string(), b"{\"a\": 1}".to_vec()), ("assets/s p a c e.bin".to_string(), big.clone()), ("assets/empty".to_string(), vec![])];
        let zip = write(&entries).unwrap();
        assert!(is_zip(&zip));
        assert!(zip.len() < big.len() / 2, "deflated");
        assert_eq!(read(&zip).unwrap(), entries);
        // damage is caught
        let mut broken = zip.clone();
        let mid = broken.len() / 2;
        broken[mid] ^= 0xFF;
        assert!(read(&broken).is_err());
        assert!(read(b"not a zip").is_err());
        assert!(read(&zip[..zip.len() - 30]).is_err());
    }
}
