import { User } from "./user";

export function formatProfile(user: User): string {
  return `${user.username} <${user.email}>`;
}
