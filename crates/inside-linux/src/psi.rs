use inside_core::observation::{CpuPsiSome, UnavailableReason};

use crate::{record, unsigned};

/// someのみを読む．項目順序は自由．未知のkey=valueは無視するが，
/// 必須項目の欠損・重複，key=valueでないトークンはエラーとする．
pub fn parse_cpu_psi_some(input: &str) -> Result<CpuPsiSome, UnavailableReason> {
    let mut values = [None; 4];
    const NAMES: [&str; 4] = ["avg10", "avg60", "avg300", "total"];
    for token in record(input, "some")?.split_whitespace().skip(1) {
        let (key, value) = token
            .split_once('=')
            .filter(|(key, value)| !key.is_empty() && !value.is_empty() && !value.contains('='))
            .ok_or_else(|| UnavailableReason::Invalid {
                field: "some",
                value: token.into(),
            })?;
        if let Some(index) = NAMES.iter().position(|name| *name == key)
            && values[index].replace(value).is_some()
        {
            return Err(UnavailableReason::Duplicate {
                field: NAMES[index],
            });
        }
    }
    let mut required = [""; 4];
    for (index, value) in values.into_iter().enumerate() {
        required[index] = value.ok_or(UnavailableReason::Missing {
            field: NAMES[index],
        })?;
    }
    Ok(CpuPsiSome {
        avg10: percentage(required[0], "avg10")?,
        avg60: percentage(required[1], "avg60")?,
        avg300: percentage(required[2], "avg300")?,
        total_us: unsigned(required[3], "total")?,
    })
}

fn percentage(value: &str, field: &'static str) -> Result<f64, UnavailableReason> {
    // カーネルの非負小数表記を検証し，NaN/inf/指数・符号表記を拒否する．
    let mut dots = 0;
    let decimal = value.bytes().all(|byte| {
        if byte == b'.' {
            dots += 1;
            true
        } else {
            byte.is_ascii_digit()
        }
    }) && dots <= 1
        && value.bytes().any(|byte| byte.is_ascii_digit());
    if decimal
        && let Ok(number) = value.parse::<f64>()
        && number.is_finite()
        && (0.0..=100.0).contains(&number)
    {
        return Ok(number);
    }
    Err(UnavailableReason::Invalid {
        field,
        value: value.into(),
    })
}
