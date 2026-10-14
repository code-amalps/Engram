package payments

type Transaction struct {
	ID        string
	Amount    int64
	Currency  string
	Status    string
	Recipient string
}
