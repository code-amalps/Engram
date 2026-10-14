package com.engram.billing;

import java.time.Instant;

public record Invoice(
    String id,
    String customerId,
    double amount,
    String status,
    Instant issuedAt
) {
    public boolean isPaid() {
        return "PAID".equalsIgnoreCase(status);
    }
}
