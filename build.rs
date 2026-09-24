//! Windows: the exe's icon (Explorer, pinned taskbar buttons, shortcuts),
//! from assets/icon/wavify.ico. The .ico becomes a compiled resource (.res)
//! here and goes straight to the linker: rust-lld reads .res files itself,
//! so no resource compiler (rc, windres) is needed.

use std::path::PathBuf;

const RT_ICON: u16 = 3;
const RT_GROUP_ICON: u16 = 14;
const LANG_EN_US: u16 = 0x0409;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/icon/wavify.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let ico = std::fs::read("assets/icon/wavify.ico").expect("read assets/icon/wavify.ico");
    let res = ico_to_res(&ico).expect("assets/icon/wavify.ico is not an icon");
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR")).join("wavify-icon.res");
    std::fs::write(&out, res).expect("write the icon resource");
    println!("cargo:rustc-link-arg-bin=wavify={}", out.display());
}

/// .ico -> .res, as `rc` compiles `1 ICON "wavify.ico"`: every image an
/// RT_ICON (ids 1..), and one RT_GROUP_ICON (id 1) listing them.
fn ico_to_res(ico: &[u8]) -> Option<Vec<u8>> {
    let u16_at = |at: usize| Some(u16::from_le_bytes(ico.get(at..at + 2)?.try_into().ok()?));
    let u32_at = |at: usize| Some(u32::from_le_bytes(ico.get(at..at + 4)?.try_into().ok()?));
    if u16_at(0)? != 0 || u16_at(2)? != 1 {
        return None;
    }
    let count = u16_at(4)?;
    let mut res = Vec::new();
    // every .res starts with an empty entry
    push_entry(&mut res, 0, 0, &[], 0, 0);
    let mut group = Vec::new();
    group.extend(0u16.to_le_bytes()); // reserved
    group.extend(1u16.to_le_bytes()); // type: icon
    group.extend(count.to_le_bytes());
    for i in 0..count as usize {
        let at = 6 + 16 * i;
        let entry = ico.get(at..at + 16)?;
        let size = u32_at(at + 8)? as usize;
        let offset = u32_at(at + 12)? as usize;
        let image = ico.get(offset..offset.checked_add(size)?)?;
        let id = i as u16 + 1;
        push_entry(&mut res, RT_ICON, id, image, 0x1010, LANG_EN_US);
        // size, colours, planes, bit count, bytes: as in the .ico; then the
        // RT_ICON id where the .ico has the image's file offset
        group.extend(&entry[..12]);
        group.extend(id.to_le_bytes());
    }
    push_entry(&mut res, RT_GROUP_ICON, 1, &group, 0x1030, LANG_EN_US);
    Some(res)
}

/// One resource: a 32-byte header (ordinal type and name), then the data,
/// padded to 4 bytes.
fn push_entry(res: &mut Vec<u8>, kind: u16, name: u16, data: &[u8], flags: u16, lang: u16) {
    res.extend((data.len() as u32).to_le_bytes()); // data size
    res.extend(32u32.to_le_bytes()); // header size
    res.extend([0xFF, 0xFF]);
    res.extend(kind.to_le_bytes());
    res.extend([0xFF, 0xFF]);
    res.extend(name.to_le_bytes());
    res.extend(0u32.to_le_bytes()); // data version
    res.extend(flags.to_le_bytes()); // memory flags
    res.extend(lang.to_le_bytes());
    res.extend(0u32.to_le_bytes()); // version
    res.extend(0u32.to_le_bytes()); // characteristics
    res.extend(data);
    while res.len() % 4 != 0 {
        res.push(0);
    }
}
