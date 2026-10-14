import { OrderService } from "../src/services/order_service";
import { OrderStatus } from "../src/models/order";

describe("OrderService", () => {
  it("should create orders in PENDING status", () => {
    const service = new OrderService();
    const order = service.createOrder("cust_123", [{ sku: "SKU-A", quantity: 2, unitPrice: 25.0 }]);
    expect(order.status).toBe(OrderStatus.Pending);
    expect(service.calculateTotal(order)).toBe(50.0);
  });
});
