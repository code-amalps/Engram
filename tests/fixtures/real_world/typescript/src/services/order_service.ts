import { Order, OrderId, OrderStatus, OrderItem } from "../models/order";
import { serializeOrdersToCsv } from "../utils/csv_serializer";

export class OrderService {
  private orders: Map<OrderId, Order> = new Map();

  public createOrder(customerId: string, items: OrderItem[]): Order {
    const id = `ord_${Date.now()}_${Math.random().toString(36).substring(2, 7)}`;
    const newOrder: Order = {
      id,
      customerId,
      items,
      status: OrderStatus.Pending,
      createdAt: new Date(),
      updatedAt: new Date()
    };
    this.orders.set(id, newOrder);
    return newOrder;
  }

  public getOrder(id: OrderId): Order | undefined {
    return this.orders.get(id);
  }

  public cancelOrder(id: OrderId, reason: string): boolean {
    const order = this.orders.get(id);
    if (!order || order.status === OrderStatus.Shipped) {
      return false;
    }
    order.status = OrderStatus.Cancelled;
    order.updatedAt = new Date();
    return true;
  }

  public calculateTotal(order: Order): number {
    return order.items.reduce((sum, item) => sum + item.quantity * item.unitPrice, 0);
  }

  public exportCsv(): string {
    const list = Array.from(this.orders.values());
    return serializeOrdersToCsv(list);
  }
}
