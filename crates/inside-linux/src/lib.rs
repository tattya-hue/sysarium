//! Linux固有の取得処理．パーサはファイルI/Oから独立して使用できる．
pub mod collector;
pub mod cpu;
pub mod psi;

use inside_core::observation::UnavailableReason;

/// 対象行を厳密な先頭トークンで選択し，重複も検出する．
fn record<'a>(input: &'a str, name: &'static str) -> Result<&'a str, UnavailableReason> {
    let mut lines = input
        .lines()
        .filter(|line| line.split_whitespace().next() == Some(name));
    let line = lines
        .next()
        .ok_or(UnavailableReason::Missing { field: name })?;
    if lines.next().is_some() {
        return Err(UnavailableReason::Duplicate { field: name });
    }
    Ok(line)
}

fn unsigned(value: &str, field: &'static str) -> Result<u64, UnavailableReason> {
    if !value.is_empty()
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && let Ok(number) = value.parse()
    {
        return Ok(number);
    }
    Err(UnavailableReason::Invalid {
        field,
        value: value.into(),
    })
}
