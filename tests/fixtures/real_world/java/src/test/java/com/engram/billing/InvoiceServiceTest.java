package com.engram.billing;

import org.junit.jupiter.api.Test;
import java.util.*;

import static org.junit.jupiter.api.Assertions.*;

class InMemoryInvoiceRepository implements InvoiceRepository {
    private final Map<String, Invoice> store = new HashMap<>();

    public Optional<Invoice> findById(String id) {
        return Optional.ofNullable(store.get(id));
    }

    public void save(Invoice invoice) {
        store.put(invoice.id(), invoice);
    }

    public List<Invoice> findByCustomerId(String customerId) {
        return store.values().stream().filter(i -> i.customerId().equals(customerId)).toList();
    }
}

public class InvoiceServiceTest {
    @Test
    void testCreateAndPayInvoice() {
        InMemoryInvoiceRepository repo = new InMemoryInvoiceRepository();
        InvoiceService service = new InvoiceService(repo);

        Invoice inv = service.createInvoice("inv_1", "cust_100", 250.0);
        assertEquals("PENDING", inv.status());

        boolean ok = service.markAsPaid("inv_1");
        assertTrue(ok);
        assertTrue(repo.findById("inv_1").get().isPaid());
    }
}
