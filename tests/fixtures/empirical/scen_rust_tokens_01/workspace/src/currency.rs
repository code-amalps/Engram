#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Currency {
    USD,
    EUR,
    GBP,
}

pub fn fx_rate_bps(from: Currency, to: Currency) -> u32 {
    if from == to {
        10_000
    } else {
        10_250
    }
}
