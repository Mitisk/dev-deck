import { describe, it, expect } from "vitest";
import { generatePassword, passwordStrength } from "../lib/password";

const CHARSET = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*-_=+";

describe("passwordStrength", () => {
  it("пустой → 0", () => {
    expect(passwordStrength("")).toBe(0);
  });
  it("слабый < сильный", () => {
    expect(passwordStrength("abc")).toBeLessThan(passwordStrength("Abcd1234!@xyzQ"));
  });
  it("длинный со всеми классами → 4", () => {
    expect(passwordStrength("Abcd1234!@xyzQ")).toBe(4);
  });
});

describe("generatePassword", () => {
  it("длина 20 по умолчанию", () => {
    expect(generatePassword(20).length).toBe(20);
  });
  it("кламп снизу до 8", () => {
    expect(generatePassword(2).length).toBe(8);
  });
  it("кламп сверху до 64", () => {
    expect(generatePassword(100).length).toBe(64);
  });
  it("только символы из CHARSET", () => {
    const pw = generatePassword(40);
    expect([...pw].every((ch) => CHARSET.includes(ch))).toBe(true);
  });
});
