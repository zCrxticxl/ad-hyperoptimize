//! Windows performance counters (PDH) by their English path, so they work on
//! every display language (PdhAddEnglishCounterW).

/// % of nominal CPU frequency (> 100 = boost).
pub const CPU_PERFORMANCE: &str = "\\Processor Information(_Total)\\% Processor Performance";
/// % of maximum frequency the CPU is currently allowed (< 100 = limited by
/// power, temperature or firmware).
pub const CPU_PERFORMANCE_LIMIT: &str = "\\Processor Information(_Total)\\% Performance Limit";
/// Share of CPU time spent in deferred procedure calls (driver work).
pub const CPU_DPC_TIME: &str = "\\Processor(_Total)\\% DPC Time";
/// Reads from disk to resolve hard page faults.
pub const HARD_FAULTS: &str = "\\Memory\\Page Reads/sec";

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;

    type HQuery = *mut c_void;
    type HCounter = *mut c_void;

    #[repr(C)]
    struct FmtValue {
        c_status: u32,
        _pad: u32,
        double_value: f64,
    }

    const PDH_FMT_DOUBLE: u32 = 0x0000_0200;

    #[link(name = "pdh")]
    extern "system" {
        fn PdhOpenQueryW(src: *const u16, user: usize, q: *mut HQuery) -> i32;
        fn PdhAddEnglishCounterW(q: HQuery, path: *const u16, user: usize, c: *mut HCounter)
            -> i32;
        fn PdhCollectQueryData(q: HQuery) -> i32;
        fn PdhGetFormattedCounterValue(
            c: HCounter,
            fmt: u32,
            ctype: *mut u32,
            val: *mut FmtValue,
        ) -> i32;
        fn PdhCloseQuery(q: HQuery) -> i32;
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub struct PerfCounter {
        query: HQuery,
        counter: HCounter,
    }
    // Raw handles are only used from the thread that owns the counter.
    unsafe impl Send for PerfCounter {}

    impl PerfCounter {
        pub fn new(path: &str) -> Option<Self> {
            unsafe {
                let mut q: HQuery = std::ptr::null_mut();
                if PdhOpenQueryW(std::ptr::null(), 0, &mut q) != 0 {
                    return None;
                }
                let path = wide(path);
                let mut c: HCounter = std::ptr::null_mut();
                if PdhAddEnglishCounterW(q, path.as_ptr(), 0, &mut c) != 0 {
                    PdhCloseQuery(q);
                    return None;
                }
                PdhCollectQueryData(q); // prime; rates need two samples
                Some(Self {
                    query: q,
                    counter: c,
                })
            }
        }

        /// Current value; call at most once per sampling interval.
        pub fn read(&self) -> Option<f64> {
            unsafe {
                if PdhCollectQueryData(self.query) != 0 {
                    return None;
                }
                let mut val = FmtValue {
                    c_status: 0,
                    _pad: 0,
                    double_value: 0.0,
                };
                let mut ctype = 0u32;
                if PdhGetFormattedCounterValue(self.counter, PDH_FMT_DOUBLE, &mut ctype, &mut val)
                    != 0
                {
                    return None;
                }
                Some(val.double_value)
            }
        }
    }

    impl Drop for PerfCounter {
        fn drop(&mut self) {
            unsafe { PdhCloseQuery(self.query) };
        }
    }
}

#[cfg(windows)]
pub use imp::PerfCounter;

/// Non-Windows builds (tests): counters are unavailable.
#[cfg(not(windows))]
pub struct PerfCounter;

#[cfg(not(windows))]
impl PerfCounter {
    pub fn new(_path: &str) -> Option<Self> {
        None
    }
    pub fn read(&self) -> Option<f64> {
        None
    }
}
