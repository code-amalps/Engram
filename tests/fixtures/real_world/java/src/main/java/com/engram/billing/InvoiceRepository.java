package com.engram.billing;

import java.util.Optional;
import java.util.List;

public interface InvoiceRepository {
    Optional<Invoice> findById(String id);
    void save(Invoice invoice);
    List<Invoice> findByCustomerId(String customerId);
}
