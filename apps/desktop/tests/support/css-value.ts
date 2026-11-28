/**
 * `value` as this CSSOM serializes it once set on `property` of an inline style.
 *
 * Equivalent CSS values have more than one spelling (`8 8 8 8 fill` and `8 fill`, `0` and `0px`), and which one a style declaration reads back is up to the CSSOM, so it changes between jsdom releases. Comparing a declaration against its expected value run through the same CSSOM checks what the component set, not how jsdom chooses to spell it.
 */
export function cssValue(property: string, value: string): string {
  const probe = document.createElement("div");
  probe.style.setProperty(property, value);
  const serialized = probe.style.getPropertyValue(property);
  if (!serialized) throw new Error(`${property}: ${value} is not a valid declaration`);
  return serialized;
}

/** The computed value of `property` on an element whose only declaration is `property: value`. */
export function computedCssValue(property: string, value: string): string {
  const probe = document.createElement("div");
  probe.style.setProperty(property, value);
  document.body.append(probe);
  try {
    return getComputedStyle(probe).getPropertyValue(property);
  } finally {
    probe.remove();
  }
}
