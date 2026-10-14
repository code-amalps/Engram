import { Order } from "../models/order";

export function serializeOrdersToCsv(orders: Order[]): string {
  const header = "id,customerId,status,itemCount,createdAt\n";
  const rows = orders.map(o => `${o.id},${o.customerId},${o.status},${o.items.length},${o.createdAt.toISOString()}`).join("\n");
  return header + rows;
}
