// Custom type definitions for Svelte 5
declare namespace svelte.JSX {
  interface HTMLAttributes<T> {
    onclick?: (event: MouseEvent) => void;
    onkeydown?: (event: KeyboardEvent) => void;
    onkeyup?: (event: KeyboardEvent) => void;
    oninput?: (event: Event) => void;
    onchange?: (event: Event) => void;
    onsubmit?: (event: SubmitEvent) => void;
    onfocus?: (event: FocusEvent) => void;
    onblur?: (event: FocusEvent) => void;
    onmouseenter?: (event: MouseEvent) => void;
    onmouseleave?: (event: MouseEvent) => void;
    onmouseover?: (event: MouseEvent) => void;
    onmouseout?: (event: MouseEvent) => void;
    onmousedown?: (event: MouseEvent) => void;
    onmouseup?: (event: MouseEvent) => void;
    ondblclick?: (event: MouseEvent) => void;
    oncontextmenu?: (event: MouseEvent) => void;
    onwheel?: (event: WheelEvent) => void;
    onscroll?: (event: Event) => void;
    onresize?: (event: Event) => void;
    onload?: (event: Event) => void;
    onerror?: (event: Event) => void;
    onabort?: (event: Event) => void;
    onbeforeunload?: (event: BeforeUnloadEvent) => void;
    onunload?: (event: Event) => void;
    ononline?: (event: Event) => void;
    onoffline?: (event: Event) => void;
    onfocusin?: (event: FocusEvent) => void;
    onfocusout?: (event: FocusEvent) => void;
    onanimationstart?: (event: AnimationEvent) => void;
    onanimationend?: (event: AnimationEvent) => void;
    onanimationiteration?: (event: AnimationEvent) => void;
    ontransitionend?: (event: TransitionEvent) => void;
    oncopy?: (event: ClipboardEvent) => void;
    oncut?: (event: ClipboardEvent) => void;
    onpaste?: (event: ClipboardEvent) => void;
    onselect?: (event: Event) => void;
    onselectstart?: (event: Event) => void;
    onselectionchange?: (event: Event) => void;
    onbeforecopy?: (event: Event) => void;
    onbeforecut?: (event: Event) => void;
    onbeforepaste?: (event: Event) => void;
    onsearch?: (event: Event) => void;
    oninvalid?: (event: Event) => void;
    onreset?: (event: Event) => void;
    onformdata?: (event: FormDataEvent) => void;
    onforminput?: (event: Event) => void;
    onformchange?: (event: Event) => void;
    onformreset?: (event: Event) => void;
    onformsubmit?: (event: Event) => void;
    onforminput?: (event: Event) => void;
    onformchange?: (event: Event) => void;
    onformreset?: (event: Event) => void;
    onformsubmit?: (event: Event) => void;
  }
}

// Extend the global HTML element interfaces
declare global {
  namespace JSX {
    interface IntrinsicElements {
      [elemName: string]: any;
    }
  }
} 