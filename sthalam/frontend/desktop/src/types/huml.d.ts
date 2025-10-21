declare module '@huml-lang/huml' {
  /**
   * Parse a HUML string into a JavaScript object
   * @param input - HUML formatted string
   * @returns Parsed object
   */
  export function parse(input: string): any;

  /**
   * Stringify a JavaScript object into HUML format
   * @param value - Object to stringify
   * @returns HUML formatted string
   */
  export function stringify(value: any): string;
}
