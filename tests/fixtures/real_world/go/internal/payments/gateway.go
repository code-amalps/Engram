package payments

type PaymentGateway interface {
	Charge(t *Transaction) error
	Refund(txID string, amount int64) error
}
