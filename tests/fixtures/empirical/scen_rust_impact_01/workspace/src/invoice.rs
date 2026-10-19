use crate::pricing::calculate_gross;
use crate::tax::TaxRate;

pub struct InvoiceItem {
    pub description: String,
    pub net_amount: u64,
    pub tax_rate: TaxRate,
}

impl InvoiceItem {
    pub fn total(&self) -> u64 {
        calculate_gross(self.net_amount, self.tax_rate)
    }
}
