use pinocchio::syscalls::sol_remaining_compute_units;
use pinocchio_log::log;

pub struct BenchmarkComputeUnit {
    name: &'static str,
    remaining_at_start: u64,
}

impl BenchmarkComputeUnit {
    pub fn start(name: &'static str) -> BenchmarkComputeUnit {
        log!("BENCHMARK BEGIN: [{}]", name);
        Self {
            name,
            remaining_at_start: Self::remaining_cu(),
        }
    }

    fn remaining_cu() -> u64 {
        unsafe { sol_remaining_compute_units() }
    }
}

impl Drop for BenchmarkComputeUnit {
    fn drop(&mut self) {
        let consumed = self.remaining_at_start - Self::remaining_cu();
        log!(
            "BENCHMARK END: [{}] consumed {} of {} compute units.",
            self.name,
            consumed,
            self.remaining_at_start
        )
    }
}

#[macro_export]
macro_rules! compute {
    ($msg:expr=> $($tt:tt)*) => {
        let _log = $crate::cu::BenchmarkComputeUnit::start($msg);
        $($tt)*
    };
}
