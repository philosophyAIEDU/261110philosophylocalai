//! 보안 점검 — 운영체제 디스크 암호화(BitLocker / FileVault) 상태 확인.
//!
//! 앱 자료는 이미 암호화되어 있지만, 내보낸 Word·Excel 파일과 다른 문서는 디스크 암호화가
//! 없으면 노트북 분실 시 그대로 노출된다. 확인할 수 없으면 "확인 불가"로 표시하고 추측하지 않는다.
use serde::Serialize;
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiskEncryption {
    On,
    Off,
    InProgress,
    Unknown,
}

/// macOS `fdesetup status` 출력 해석.
pub fn parse_fdesetup(out: &str) -> DiskEncryption {
    let o = out.to_ascii_lowercase();
    if o.contains("encryption in progress") || o.contains("decryption in progress") {
        DiskEncryption::InProgress
    } else if o.contains("filevault is on") {
        DiskEncryption::On
    } else if o.contains("filevault is off") {
        DiskEncryption::Off
    } else {
        DiskEncryption::Unknown
    }
}

/// Windows 셸 속성 `System.Volume.BitLockerProtection` 값 해석 (관리자 권한 없이 읽을 수 있음).
/// 1 = 켜짐, 2 = 꺼짐, 3 = 암호화 중, 6 = 켜짐(잠김). 그 밖의 값은 확인 불가로 둔다.
pub fn parse_bitlocker_protection(out: &str) -> DiskEncryption {
    match out.trim() {
        "1" | "6" => DiskEncryption::On,
        "2" => DiskEncryption::Off,
        "3" => DiskEncryption::InProgress,
        _ => DiskEncryption::Unknown,
    }
}

/// Linux `lsblk -rno TYPE,MOUNTPOINT` 출력에서 루트(/)가 암호화 장치(crypt) 위에 있는지.
pub fn parse_lsblk(out: &str) -> DiskEncryption {
    // lsblk는 트리 구조를 잃으므로, crypt 장치가 / 에 마운트되어 있거나 crypt가 존재하고 / 가 lvm 위에 있으면 켜짐으로 본다.
    let lines: Vec<(&str, &str)> = out.lines().filter_map(|l| {
        let mut it = l.split_whitespace();
        Some((it.next()?, it.next().unwrap_or("")))
    }).collect();
    let has_crypt = lines.iter().any(|(t, _)| *t == "crypt");
    let root_on_crypt = lines.iter().any(|(t, m)| *t == "crypt" && *m == "/");
    let root_on_lvm = lines.iter().any(|(t, m)| *t == "lvm" && *m == "/");
    if root_on_crypt || (has_crypt && root_on_lvm) {
        DiskEncryption::On
    } else {
        DiskEncryption::Unknown
    }
}

fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let mut c = Command::new(cmd);
    c.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    let o = c.output().ok()?;
    Some(String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr))
}

pub fn disk_encryption() -> DiskEncryption {
    #[cfg(target_os = "macos")]
    {
        run("fdesetup", &["status"]).map(|s| parse_fdesetup(&s)).unwrap_or(DiskEncryption::Unknown)
    }
    #[cfg(target_os = "windows")]
    {
        let script = "$d=$env:SystemDrive; (New-Object -ComObject Shell.Application).NameSpace($d + '\\').Self.ExtendedProperty('System.Volume.BitLockerProtection')";
        run("powershell.exe", &["-NoProfile", "-NonInteractive", "-Command", script]).map(|s| parse_bitlocker_protection(&s)).unwrap_or(DiskEncryption::Unknown)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        run("lsblk", &["-rno", "TYPE,MOUNTPOINT"]).map(|s| parse_lsblk(&s)).unwrap_or(DiskEncryption::Unknown)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsers() {
        assert_eq!(parse_fdesetup("FileVault is On.\n"), DiskEncryption::On);
        assert_eq!(parse_fdesetup("FileVault is Off.\n"), DiskEncryption::Off);
        assert_eq!(parse_fdesetup("FileVault is On.\nEncryption in progress: Percent completed = 12.0"), DiskEncryption::InProgress);
        assert_eq!(parse_fdesetup("error"), DiskEncryption::Unknown);
        assert_eq!(parse_bitlocker_protection("1\r\n"), DiskEncryption::On);
        assert_eq!(parse_bitlocker_protection("2"), DiskEncryption::Off);
        assert_eq!(parse_bitlocker_protection(""), DiskEncryption::Unknown);
        assert_eq!(parse_lsblk("disk \npart /boot\ncrypt \nlvm /\n"), DiskEncryption::On);
        assert_eq!(parse_lsblk("disk \npart /\n"), DiskEncryption::Unknown);
    }

    #[test]
    fn detection_does_not_panic() {
        let _ = disk_encryption();
    }
}
