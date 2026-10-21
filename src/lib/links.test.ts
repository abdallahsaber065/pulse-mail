import { describe, expect, it } from "vitest";
import { baseDomain, checkLink } from "./links";

describe("links", () => {
  it("finds base domains", () => {
    expect(baseDomain("www.paypal.com")).toBe("paypal.com");
    expect(baseDomain("a.b.amazon.co.uk")).toBe("amazon.co.uk");
    expect(baseDomain("shop.amazon.com.eg")).toBe("amazon.com.eg");
  });

  it("flags a link whose text shows another domain", () => {
    const c = checkLink("https://paypa1-secure.test/login", "https://www.paypal.com/signin")!;
    expect(c.mismatch).toBe(true);
    expect(c.shown).toBe("www.paypal.com");
    expect(c.host).toBe("paypa1-secure.test");
  });

  it("accepts tracking redirects on the same site and plain-text links", () => {
    expect(checkLink("https://click.github.com/x?y", "github.com/settings")!.mismatch).toBe(false);
    expect(checkLink("https://example.com/a", "Read more")!.mismatch).toBe(false);
  });

  it("notices punycode and http", () => {
    const c = checkLink("http://xn--pple-43d.com/", "Apple")!;
    expect(c.punycode).toBe(true);
    expect(c.insecure).toBe(true);
  });

  it("ignores non-web links", () => {
    expect(checkLink("mailto:a@b.test", "a@b.test")).toBeNull();
    expect(checkLink("not a url", "x")).toBeNull();
  });
});
