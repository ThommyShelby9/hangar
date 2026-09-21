//! Windows performance counters (PDH), added by their English names so they work on a French
//! Windows too. Wildcard counters are re-expanded on every collection (new processes, new GPUs).

use std::ffi::c_void;

use windows_sys::Win32::System::Performance::{
    PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW,
    PdhOpenQueryW, PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE,
};

const PDH_MORE_DATA: u32 = 0x800007D2;
/// Percentages above 100 are legitimate for multi-engine GPU counters.
const PDH_FMT_NOCAP100: u32 = 0x8000;

pub struct Query {
    query: *mut c_void,
}

// Only used behind the hardware monitor's mutex.
unsafe impl Send for Query {}

pub struct Counter(*mut c_void);
unsafe impl Send for Counter {}

impl Query {
    pub fn open() -> Option<Self> {
        let mut query = std::ptr::null_mut();
        let status = unsafe { PdhOpenQueryW(std::ptr::null(), 0, &mut query) };
        (status == 0).then_some(Self { query })
    }

    /// `None` when the counter doesn't exist on this machine (no thermal zone, old Windows…).
    pub fn add(&self, english_path: &str) -> Option<Counter> {
        let path: Vec<u16> = english_path.encode_utf16().chain(std::iter::once(0)).collect();
        let mut counter = std::ptr::null_mut();
        let status = unsafe { PdhAddEnglishCounterW(self.query, path.as_ptr(), 0, &mut counter) };
        (status == 0).then_some(Counter(counter))
    }

    /// Takes a sample. Rate counters (%, bytes/s) need two samples before they report values.
    pub fn collect(&self) {
        unsafe {
            PdhCollectQueryData(self.query);
        }
    }

    /// Every instance of a counter from the last sample: (instance name, value).
    pub fn values(&self, counter: &Counter) -> Vec<(String, f64)> {
        let format = PDH_FMT_DOUBLE | PDH_FMT_NOCAP100;
        let mut size = 0u32;
        let mut count = 0u32;
        unsafe {
            let status = PdhGetFormattedCounterArrayW(counter.0, format, &mut size, &mut count, std::ptr::null_mut());
            if status != PDH_MORE_DATA || size == 0 {
                return vec![];
            }
            // The buffer holds the items followed by their name strings.
            let item_size = std::mem::size_of::<PDH_FMT_COUNTERVALUE_ITEM_W>();
            let mut buffer = vec![0u8; size as usize + item_size];
            let items = buffer.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W;
            if PdhGetFormattedCounterArrayW(counter.0, format, &mut size, &mut count, items) != 0 {
                return vec![];
            }
            (0..count as usize)
                .map(|i| &*items.add(i))
                .filter(|item| item.FmtValue.CStatus == 0)
                .map(|item| (wide_to_string(item.szName), item.FmtValue.Anonymous.doubleValue))
                .collect()
        }
    }
}

impl Drop for Query {
    fn drop(&mut self) {
        unsafe {
            PdhCloseQuery(self.query);
        }
    }
}

unsafe fn wide_to_string(ptr: *const u16) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let len = (0..).take_while(|&i| *ptr.add(i) != 0).count();
    String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len))
}
