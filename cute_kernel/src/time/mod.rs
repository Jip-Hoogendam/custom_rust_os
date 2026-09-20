mod clock;

use core::{
    ops::{Add, AddAssign},
    time::Duration,
};

pub use clock::{Clock, SYSTEM_CLOCK};

use crate::{arch::port::Port, variables::DESIRED_CLOCK_FREAKUENCY};

const NANOS_PER_SEC: u64 = 1_000_000_000;

///A time instant based on rust `std::time::Instant`
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Instant {
    nano_seconds: u32,
    seconds: u64,
}

impl AddAssign<Duration> for Instant {
    #[allow(clippy::cast_possible_truncation)]
    fn add_assign(&mut self, rhs: Duration) {
        self.seconds += rhs.as_secs();

        self.nano_seconds += (rhs.as_nanos() % u128::from(NANOS_PER_SEC)) as u32;
        #[allow(clippy::cast_possible_truncation)]
        if self.nano_seconds > NANOS_PER_SEC as u32 {
            self.nano_seconds %= NANOS_PER_SEC as u32;
            self.nano_seconds += 1;
        }
    }
}

impl Add<Duration> for Instant {
    type Output = Self;
    fn add(self, rhs: Duration) -> Self::Output {
        let mut seconds = self.seconds + rhs.as_secs();

        #[allow(clippy::cast_possible_truncation)]
        let mut nano_seconds =
            self.nano_seconds + (rhs.as_nanos() % u128::from(NANOS_PER_SEC)) as u32;
        #[allow(clippy::cast_possible_truncation)]
        if nano_seconds > NANOS_PER_SEC as u32 {
            nano_seconds %= NANOS_PER_SEC as u32;
            seconds += 1;
        }
        Self {
            nano_seconds,
            seconds,
        }
    }
}

impl Instant {
    pub fn now() -> Self {
        let ticks = SYSTEM_CLOCK.progress();
        let tps = SYSTEM_CLOCK.tps();

        let seconds = ticks / tps as u64;
        let nano_seconds = (ticks * (NANOS_PER_SEC / tps as u64)) % NANOS_PER_SEC;

        #[allow(clippy::cast_possible_truncation)]
        Self {
            nano_seconds: nano_seconds as u32,
            seconds,
        }
    }

    #[must_use]
    pub fn since(&self, other: Instant) -> Duration {
        let (mut nano_seconds, overflow) = other.nano_seconds.overflowing_sub(self.nano_seconds);

        #[allow(clippy::cast_possible_truncation)]
        let seconds = if overflow {
            nano_seconds =
                NANOS_PER_SEC.saturating_sub_signed(i64::from(nano_seconds.cast_signed())) as u32;
            other.seconds.saturating_sub(self.seconds + 1)
        } else {
            other.seconds.saturating_sub(self.seconds)
        };

        Duration::from_nanos(u64::from(nano_seconds)) + Duration::from_secs(seconds)
    }

    #[must_use]
    pub fn elapsed(&self) -> Duration {
        let now = Self::now();

        self.since(now)
    }
}

/// Sets up the system clock source to the 8253PIT.
/// Code based off : <http://www.osdever.net/bkerndev/Docs/pit.html>
pub fn set_system_clock_source_8253pit() {
    const BASE_FREQ_8253PIT: usize = 119_3182; //hz
    #[allow(clippy::cast_possible_truncation)]
    const RES_FREQUENCY_DIVISOR: u16 = (BASE_FREQ_8253PIT / DESIRED_CLOCK_FREAKUENCY) as u16;
    const DIVISOR_LOW_BYTE: u8 = (RES_FREQUENCY_DIVISOR & 0xFF) as u8;
    const DIVISOR_HIGH_BYTE: u8 = (RES_FREQUENCY_DIVISOR >> 8) as u8;
    //sets up square wave generator
    //hi/lo byte access mode
    //on channel 0
    const CONFIG_CHANNEL_8253PIT: u8 = 0x36;
    const REAL_FREQUENCY: usize = BASE_FREQ_8253PIT / RES_FREQUENCY_DIVISOR as usize;

    const {
        assert!(DESIRED_CLOCK_FREAKUENCY > 0);
        assert!(DESIRED_CLOCK_FREAKUENCY <= BASE_FREQ_8253PIT);
        assert!(BASE_FREQ_8253PIT / DESIRED_CLOCK_FREAKUENCY < u16::MAX as usize);
    }

    unsafe {
        //Divide by 2 because it triggers on both rising and falling edge of the frequency
        SYSTEM_CLOCK.set_tps(REAL_FREQUENCY / 2);
    }

    let mut command_port = Port::new(0x43);
    let mut channel_0_port = Port::new(0x40);
    command_port.write(CONFIG_CHANNEL_8253PIT);

    channel_0_port.write(DIVISOR_LOW_BYTE);
    channel_0_port.write(DIVISOR_HIGH_BYTE);
}
