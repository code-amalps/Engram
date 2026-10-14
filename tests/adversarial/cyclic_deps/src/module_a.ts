import { HelperB } from "./module_b";

export class HelperA {
  public execute(): string {
    const b = new HelperB();
    return "A -> " + b.name;
  }
}
