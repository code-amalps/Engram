import { HelperA } from "./module_a";

export class HelperB {
  public name: string = "HelperB";

  public callA(): string {
    const a = new HelperA();
    return "B -> " + a.execute();
  }
}
