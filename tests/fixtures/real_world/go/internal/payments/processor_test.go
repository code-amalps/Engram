package payments

import "testing"

type mockGateway struct{}

func (m *mockGateway) Charge(t *Transaction) error { return nil }
func (m *mockGateway) Refund(id string, amt int64) error { return nil }

func TestPaymentProcessor(t *testing.T) {
	proc := &PaymentProcessor{Gateway: &mockGateway{}}
	tx := &Transaction{ID: "tx_1", Amount: 100, Currency: "USD"}
	if err := proc.Process(tx); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if tx.Status != "COMPLETED" {
		t.Fatalf("expected COMPLETED, got %s", tx.Status)
	}
}
