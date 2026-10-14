package payments

import "fmt"

type PaymentProcessor struct {
	Gateway PaymentGateway
}

func (p *PaymentProcessor) Process(t *Transaction) error {
	if t.Amount <= 0 {
		return fmt.Errorf("invalid payment amount: %d", t.Amount)
	}
	t.Status = "PROCESSING"
	if err := p.Gateway.Charge(t); err != nil {
		t.Status = "FAILED"
		return err
	}
	t.Status = "COMPLETED"
	return nil
}
