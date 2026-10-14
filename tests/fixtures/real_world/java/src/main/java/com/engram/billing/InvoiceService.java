package com.engram.billing;

import java.time.Instant;
import java.util.Optional;

public class InvoiceService {
    private final InvoiceRepository repository;

    public InvoiceService(InvoiceRepository repository) {
        this.repository = repository;
    }

    public Invoice createInvoice(String id, String customerId, double amount) {
        Invoice invoice = new Invoice(id, customerId, amount, "PENDING", Instant.now());
        repository.save(invoice);
        return invoice;
    }

    public boolean markAsPaid(String id) {
        Optional<Invoice> opt = repository.findById(id);
        if (opt.isEmpty()) {
            return false;
        }
        Invoice current = opt.get();
        Invoice paid = new Invoice(current.id(), current.customerId(), current.amount(), "PAID", current.issuedAt());
        repository.save(paid);
        return true;
    }
}
