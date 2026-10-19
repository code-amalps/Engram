#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaxRate {
    Standard,
    Reduced,
    Zero,
}

pub fn rate_percentage(rate: TaxRate) -> u32 {
    match rate {
        TaxRate::Standard => 20,
        TaxRate::Reduced => 5,
        TaxRate::Zero => 0,
    }
}
