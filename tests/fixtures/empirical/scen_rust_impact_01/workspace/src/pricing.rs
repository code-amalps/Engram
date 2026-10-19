use crate::tax::{rate_percentage, TaxRate};

pub fn calculate_gross(net_cents: u64, rate: TaxRate) -> u64 {
    let pct = rate_percentage(rate) as u64;
    net_cents + (net_cents * pct) / 100
}
