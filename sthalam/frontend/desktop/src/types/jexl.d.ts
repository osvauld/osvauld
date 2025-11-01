declare module 'jexl' {
  interface Jexl {
    eval(expression: string, context?: any): Promise<any>;
    evalSync(expression: string, context?: any): any;
    addTransform(name: string, transform: (...args: any[]) => any): void;
    addFunction(name: string, fn: (...args: any[]) => any): void;
    addBinaryOp(operator: string, precedence: number, fn: (left: any, right: any) => any): void;
    removeOp(operator: string): void;
  }

  const jexl: Jexl;
  export default jexl;
}
