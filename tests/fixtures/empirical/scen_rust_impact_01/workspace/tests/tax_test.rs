use fixture_tax::invoice::InvoiceItem;
use fixture_tax::pricing::calculate_gross;
use fixture_tax::tax::TaxRate;

#[test]
fn test_pricing() {
    assert_eq!(calculate_gross(100, TaxRate::Standard), 120);
    assert_eq!(calculate_gross(100, TaxRate::Reduced), 105);
    assert_eq!(calculate_gross(100, TaxRate::Zero), 100);
}

#[test]
fn test_invoice() {
    let item = InvoiceItem {
        description: "Service".into(),
        net_amount: 500,
        tax_rate: TaxRate::Standard,
    };
    assert_eq!(item.total(), 600);
}
