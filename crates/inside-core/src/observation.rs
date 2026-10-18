/// 累積CPU時間．単位は取得元のクロックtick（USER_HZ）．
/// guest / guest_niceはuser / niceと重複するため保持しない．
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuTimes {
    pub user: u64,
    pub nice: u64,
    pub system: u64,
    pub idle: u64,
    pub iowait: u64,
    pub irq: u64,
    pub softirq: u64,
    pub steal: u64,
}

/// CPU競合の平滑化された傾向（百分率）と累積停滞時間．
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CpuPsiSome {
    pub avg10: f64,
    pub avg60: f64,
    pub avg300: f64,
    pub total_us: u64,
}

/// 欠測を0で代用せず，取得・パース失敗の理由を保持する．
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnavailableReason {
    FirstSample,
    MissingInterval,
    ZeroInterval,
    ZeroCpuDelta,
    CounterRegression {
        field: &'static str,
    },
    ArithmeticOverflow,
    OutOfRange {
        field: &'static str,
    },
    InvalidSampleOrder,
    ScopeChanged,
    SourceChanged,
    InputUnavailable {
        sample_id: u64,
        source: &'static str,
        reason: Box<UnavailableReason>,
    },
    NotFound,
    PermissionDenied,
    Io {
        message: String,
    },
    Missing {
        field: &'static str,
    },
    Invalid {
        field: &'static str,
        value: String,
    },
    Duplicate {
        field: &'static str,
    },
}

/// 値が利用できない場合も取得元を保持する．
#[derive(Debug, Clone, PartialEq)]
pub struct Sourced<T> {
    pub source: &'static str,
    pub value: Result<T, UnavailableReason>,
}

/// 取得元の生データ．観測メタデータや差分計算とは分離する．
#[derive(Debug, Clone, PartialEq)]
pub struct RawCpuReadings {
    pub cpu_times: Sourced<CpuTimes>,
    pub cpu_psi_some: Sourced<CpuPsiSome>,
    /// 実行中を含むrunnableの参考値．正確な待機タスク数ではない．
    pub procs_running: Sourced<u64>,
}

/// 壁時計は表示専用．差分の区間には単調時計から得たDurationだけを使う．
#[derive(Debug, Clone, PartialEq)]
pub struct Observation {
    pub sample_id: u64,
    pub sampled_at_unix_ms: u64,
    pub elapsed_since_previous: Option<std::time::Duration>,
    pub scope: &'static str,
    pub readings: RawCpuReadings,
}

pub const SYSTEM_VIEW: &str = "system-view";
