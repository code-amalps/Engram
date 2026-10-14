package broken

// Missing import and unresolved symbols test case
type IncompleteHandler struct {
	ID string
}

func (h *IncompleteHandler) Process() error {
	return nil
}
